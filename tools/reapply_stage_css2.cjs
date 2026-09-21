const fs = require("fs");
const p = "C:\\Users\\32875\\Desktop\\应用快照\\prototype\\src\\styles\\prototype.css";
let css = fs.readFileSync(p, "utf8");
css = css.replace(/\r\n/g, "\n");

css = css.replace(/\.viewport-avatar-center \{[^}]*\}/, [
".viewport-avatar-center {",
"  position: relative;",
"  z-index: 10;",
"  display: flex;",
"  align-items: center;",
"  justify-content: center;",
"  width: 100%;",
"  height: 100%;",
"}",
].join("\n"));

css = css.replace(/\.cat-stage-orb \{[^}]*\}/, [
".cat-stage-orb {",
"  width: auto;",
"  height: auto;",
"  max-width: 100%;",
"  max-height: 100%;",
"  border-radius: 0;",
"  overflow: visible;",
"  box-shadow: none;",
"  border: 0;",
"  background: transparent;",
"  display: grid;",
"  place-items: center;",
"  transition: opacity 0.15s var(--ease-smooth), transform 0.15s var(--ease-smooth);",
"}",
].join("\n"));

css = css.replace(/\[data-theme="light"\] \.cat-stage-orb \{[^}]*\}/, [
'[data-theme="light"] .cat-stage-orb {',
"  border: 0;",
"  box-shadow: none;",
"  background: transparent;",
"}",
].join("\n"));

css = css.replace(/\.stage-video-element, \.stage-image-element \{[^}]*\}/, [
".stage-video-element, .stage-image-element {",
"  width: 100%;",
"  height: 100%;",
"  object-fit: contain;",
"  display: block;",
"}",
].join("\n"));

fs.writeFileSync(p, css, "utf8");
console.log("done");
