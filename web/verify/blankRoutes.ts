// The app's own routes with a blank page for each, for a test of what moves between them: nothing is loaded when it does.
import type { RouteRecordRaw } from 'vue-router';
import { routes } from '../src/router';

const blank = { render: () => null };
export const blankRoutes: RouteRecordRaw[] = routes.map((r) => (r.redirect ? r : ({ ...r, component: blank } as RouteRecordRaw)));
