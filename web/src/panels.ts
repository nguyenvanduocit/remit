// How the resizable columns start and where the owner can drag them (see ui/resizable). A list at the side (of rows, of the
// files of a skill or of a change) is sized in pixels and keeps its width when the window changes. The agent drawer, where
// the owner reads what an agent is doing, takes a share of the screen, so a wide screen gives it room. The main column
// takes the rest and never gives up more than this share of the screen.
export const listSidebar = { sizeUnit: 'px', defaultSize: 320, minSize: 280, maxSize: 480 } as const;
export const cardDrawer = { sizeUnit: '%', defaultSize: 30, minSize: 25, maxSize: 60 } as const;
export const fileList = { sizeUnit: 'px', defaultSize: 300, minSize: 160, maxSize: 600 } as const;
// Where the file list does not leave the file room beside it (the pane under 600px), it goes above it instead, a share of the height.
export const fileListAbove = { sizeUnit: '%', defaultSize: 30, minSize: 15, maxSize: 60 } as const;
export const STACK_FILES_BELOW = 600;
export const MAIN_MIN_PERCENT = 35;
// Under COMPACT_BELOW pixels of page (the window, less the Assistant's column while it is open) the three columns of a page
// with a drawer do not fit: the list (280px at least) and a drawer of a third of the page leave the open page and the drawer
// too narrow to read (a name cut to "C..", a tab to "Sessio"). So while something is open the list gives way, its page one
// click away in the top bar, and the drawer takes half the page (never under 45%: 360px at 800px, the narrowest window the
// app is made for).
export const COMPACT_BELOW = 1200;
export const compactDrawer = { sizeUnit: '%', defaultSize: 50, minSize: 45, maxSize: 65 } as const;
// The Assistant's column, at the right of every view while it is open, from under the top bar to the window's foot. Sized in
// pixels like a list at the side: a wider window gives the view the room, not the conversation. The owner drags its left edge.
export const assistantColumn = { sizeUnit: 'px', defaultSize: 420, minSize: 320, maxSize: 720 } as const;

// Narrow panes keep their full width and split the available height.
export const paneStackBelow = 720;
export const stackedList = { sizeUnit: '%', defaultSize: 30, minSize: 15, maxSize: 60 } as const;
export const stackedAssistant = { sizeUnit: '%', defaultSize: 50, minSize: 30, maxSize: 65 } as const;
