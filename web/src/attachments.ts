import { reactive } from 'vue';
import type { Attachment, AttachmentRef } from './core/generated/types.generated';
import { api } from './replica';
import { newDraftId } from './drafts';

// A model sees no more than this many pixels on an image's long side: a larger one costs bytes and shows it nothing more.
const FIT = 1568;
// What the server keeps of one image at most (`ATTACHMENT_MAX_BYTES`).
const MAX_BYTES = 5 * 1024 * 1024;
const INLINE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];
export const isInlineImage = (type: string): boolean => INLINE_TYPES.includes(type);

// Data URLs of the images this page put or read, by id; a read on its way is null, one that failed empty.
const shown = reactive(new Map<string, string | null>());

const dataUrl = (i: Attachment): string => `data:${i.mediaType};base64,${i.data}`;

function base64(blob: Blob): Promise<string> {
  return new Promise((done, fail) => {
    const r = new FileReader();
    r.onload = () => done(String(r.result).replace(/^data:[^,]*,/, ''));
    r.onerror = () => fail(r.error ?? new Error('the file could not be read'));
    r.readAsDataURL(blob);
  });
}

// The file as a model takes it: as it is when it fits, else drawn again at FIT on its long side as WebP. A GIF that fits
// stays one, its frames with it.
async function fitted(file: File): Promise<Pick<Attachment, 'mediaType' | 'data'>> {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, FIT / Math.max(bitmap.width, bitmap.height));
  if (scale === 1 && file.size <= MAX_BYTES) {
    bitmap.close();
    return { mediaType: file.type, data: await base64(file) };
  }
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  canvas.getContext('2d')?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const blob = await new Promise<Blob | null>((done) => canvas.toBlob(done, 'image/webp', 0.9));
  if (!blob) throw new Error(`${file.name} could not be drawn as an image`);
  return { mediaType: 'image/webp', data: await base64(blob) };
}

/** Uploads one file and returns the reference saved in the draft. */
export async function putAttachment(file: File, home: string): Promise<AttachmentRef> {
  const inline = isInlineImage(file.type);
  const preview = inline ? await fitted(file).catch(() => undefined) : undefined;
  const mediaType = preview?.mediaType ?? (inline ? 'application/octet-stream' : file.type || 'application/octet-stream');
  if (!preview && file.size > MAX_BYTES) throw new Error('The file exceeds 5 MiB');
  const attachment = { name: file.name || 'Attachment', mediaType, data: preview?.data ?? await base64(file) };
  const id = `att_${newDraftId()}`;
  await api.attachmentPut(id, home, attachment);
  if (preview) shown.set(id, dataUrl(attachment));
  return { id, name: attachment.name, mediaType };
}

/** The image's data URL to show, read the first time it is asked; empty until it is here. */
export function imageUrl(id: string): string {
  const held = shown.get(id);
  if (held !== undefined) return held ?? '';
  shown.set(id, null);
  api.attachmentGet(id).then(
    (i) => shown.set(id, isInlineImage(i.mediaType) ? dataUrl(i) : ''),
    () => shown.set(id, ''),
  );
  return '';
}
