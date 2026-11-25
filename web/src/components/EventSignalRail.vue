<script setup lang="ts">
import { useAct } from '../act';
import Icon from './Icon.vue';
import Tip from './Tip.vue';
import { Button } from '@/components/ui/button';

// The owner's rail on one event signal: pause it (it takes no event from any writer, its door answers 401) or resume it. The
// same action the Assistant's pause_event_signal and resume_event_signal take for him; where it shows (the signal's page) names the signal. A button's name starts with the words it shows, so the owner can say them
// to reach it.
defineProps<{ id: string; name: string; paused: boolean }>();
const act = useAct();
</script>

<template>
  <Tip v-if="paused" text="Take its events again, from every writer"><Button variant="outline" size="sm" :aria-label="`Resume event signal ${name}`" data-resume @click="act('eventSignalResume', [id], 'Event signal resumed')"><Icon name="play" :size="12" /> Resume</Button></Tip>
  <Tip v-else text="Take no event from any writer until you resume it: its door answers 401, its sensors and agents are refused"><Button variant="outline" size="sm" :aria-label="`Pause event signal ${name}`" data-pause @click="act('eventSignalPause', [id], 'Event signal paused')"><Icon name="pause" :size="12" /> Pause</Button></Tip>
</template>
