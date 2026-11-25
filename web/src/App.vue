<script setup lang="ts">
import { computed } from 'vue';
import { session } from './session';
import { oneOf } from './core/client';
import { useModel } from './replica';
import type { Who } from './core/generated/types.generated';
import Login from './components/Login.vue';
import MentorShell from './components/MentorShell.vue';
import Shell from './components/Shell.vue';

// The way in until this browser is signed in, then the app of who the server says it came in as (Snapshot.who): the
// owner's, or a mentor's (ADR-0089).
const who = useModel<Who>({ model: 'who' });
const mentor = computed(() => oneOf(who)?.kind === 'mentor');
</script>

<template>
  <Login v-if="session.auth !== 'ok'" />
  <MentorShell v-else-if="mentor" />
  <Shell v-else />
</template>
