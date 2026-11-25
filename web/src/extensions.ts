// What the owner sees of the plugins that are sources, and the words he says to install one.

import type { ExtensionOrigin } from './core/generated/types.generated';

// The sentence the owner finishes with the path of a built plugin when he installs one through the Assistant.
export const INSTALL_PLUGIN_WORDS = 'Install the plugin at ';

// Where a plugin came from, in the owner's words.
export const ORIGIN_LABEL: Record<ExtensionOrigin, string> = {
  bundled: 'Ships with Remit',
  program: 'A program you applied',
  api: 'An API behind the proxy',
  mcp: 'An MCP server you applied',
};
