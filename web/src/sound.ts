// A short sound when something new waits on the owner (notify.ts), off until he turns it on from the palette (⌘K, "Play a
// sound…"). Whether it plays is this browser's preference, kept where the browser keeps it, as it keeps whether it may notify:
// view state, not the server's, and no setting screen. The sound is made here, with the Web Audio API: two soft tones a fifth
// apart, a fifth of a second, no file to ship. A browser plays sound only after a gesture on the page: the first press after a
// reload wakes it.
import { ref } from 'vue';

// In this browser's storage, not the URL nor the server: a device setting, like the notification permission notify.ts uses.
const KEY = 'remit.sound';
export const soundSupported = (): boolean => typeof AudioContext !== 'undefined';
// Whether the sound is on, in this browser.
export const soundOn = ref(soundSupported() && localStorage.getItem(KEY) === '1');

let ctx: AudioContext | undefined;
const audio = (): AudioContext => (ctx ??= new AudioContext());
// The first press on the page lets the browser play: it is woken then, once.
if (soundOn.value) {
  const wake = () => void audio().resume();
  window.addEventListener('pointerdown', wake, { once: true, capture: true });
  window.addEventListener('keydown', wake, { once: true, capture: true });
}

// Plays the sound of something new waiting.
export function chime(): void {
  if (!soundSupported()) return;
  const a = audio();
  const at = a.currentTime;
  for (const [hz, start] of [
    [660, 0],
    [990, 0.09],
  ] as const) {
    const tone = a.createOscillator();
    const gain = a.createGain();
    tone.type = 'sine';
    tone.frequency.value = hz;
    gain.gain.setValueAtTime(0, at + start);
    gain.gain.linearRampToValueAtTime(0.08, at + start + 0.01);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + start + 0.16);
    tone.connect(gain).connect(a.destination);
    tone.start(at + start);
    tone.stop(at + start + 0.18);
  }
}

// Turns the sound on or off in this browser; called from the owner's gesture, so turning it on plays it once, as he will hear it.
export function setSound(on: boolean): void {
  soundOn.value = on;
  localStorage.setItem(KEY, on ? '1' : '0');
  if (on) chime();
}
