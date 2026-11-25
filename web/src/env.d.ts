declare module '*.vue' {
  import type { DefineComponent } from 'vue';
  const component: DefineComponent<object, object, any>;
  export default component;
}

/** The name of this build of the app (vite.config.ts); empty on the dev server, which runs the source. */
declare const __BUNDLE__: string;
