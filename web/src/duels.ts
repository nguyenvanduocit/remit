// When duel standings may have moved, for the Duels page to read them again: the core says so (a judge's pick, an
// overturn) and it counts once. The standings themselves stay the server's (duelStandings); this only says when to ask.
import { ref } from 'vue';

const moves = ref(0);
export const duelMoves = (): number => moves.value;

export function noteDuelMove(): void {
  moves.value++;
}
