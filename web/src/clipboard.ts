// Puts text on the clipboard, and says whether it went there. A page that is not served securely has no clipboard to write to.
import { toast } from './act';

export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    toast('Copied');
  } catch {
    toast('Could not copy: select the text and copy it yourself');
  }
}
