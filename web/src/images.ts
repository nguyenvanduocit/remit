// Images in the owner's messages. A picked, pasted or dropped image is fitted to what a model reads (its long side at most
// FIT pixels, a kind it takes), put on the server by an id the page makes (`image_put`), and named by the draft; the
// server keeps the bytes, and a draft or a message names them (`ImageRef`). A page shows an image from what it put, else
// reads it once (`image_get`).
import { reactive } from 'vue';
import type { Image, ImageRef, ImageType } from './core/generated/types.generated';
import { api } from './replica';
import { newDraftId } from './drafts';

// A model sees no more than this many pixels on an image's long side: a larger one costs bytes and shows it nothing more.
const FIT = 1568;
// What the server keeps of one image at most (`IMAGE_MAX_BYTES`).
const MAX_BYTES = 5 * 1024 * 1024;
const TAKEN: readonly ImageType[] = ['image/png', 'image/jpeg', 'image/gif', 'image/webp'];
const isTaken = (t: string): t is ImageType => TAKEN.some((x) => x === t);

// Data URLs of the images this page put or read, by id; a read on its way is null, one that failed empty.
const shown = reactive(new Map<string, string | null>());

const dataUrl = (i: Image): string => `data:${i.mediaType};base64,${i.data}`;

function base64(blob: Blob): Promise<string> {
  return new Promise((done, fail) => {
    const r = new FileReader();
    r.onload = () => done(String(r.result).replace(/^data:[^,]*,/, ''));
    r.onerror = () => fail(r.error ?? new Error('the image could not be read'));
    r.readAsDataURL(blob);
  });
}

// The file as a model takes it: as it is when it fits, else drawn again at FIT on its long side as WebP. A GIF that fits
// stays one, its frames with it.
async function fitted(file: File): Promise<Image> {
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, FIT / Math.max(bitmap.width, bitmap.height));
  if (isTaken(file.type) && scale === 1 && file.size <= MAX_BYTES) {
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

/** The images among files, put for a draft of home: what the draft names them by. */
export async function putImages(files: readonly File[], home: string): Promise<ImageRef[]> {
  const out: ImageRef[] = [];
  for (const file of files.filter((f) => f.type.startsWith('image/'))) {
    const image = await fitted(file);
    const id = `im_${newDraftId()}`;
    await api.imagePut(id, home, image);
    shown.set(id, dataUrl(image));
    out.push({ id, mediaType: image.mediaType });
  }
  return out;
}

/** The image's data URL to show, read the first time it is asked; empty until it is here. */
export function imageUrl(id: string): string {
  const held = shown.get(id);
  if (held !== undefined) return held ?? '';
  shown.set(id, null);
  api.imageGet(id).then(
    (i) => shown.set(id, dataUrl(i)),
    () => shown.set(id, ''),
  );
  return '';
}
