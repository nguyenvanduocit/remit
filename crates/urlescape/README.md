# urlescape

A text put into a URL, and read back out of one: a path segment, a query value, and a `%XX` escape decoded. Every crate that
writes an outside id or a value into a request (the forge, the sources, the bundled plugins, `remit emit`) escapes it here. It
is pure and depends on no crate.
