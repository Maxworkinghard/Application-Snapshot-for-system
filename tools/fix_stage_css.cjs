const fs = require("fs");
const cssPath = "C:\\Users\\32875\\Desktop\\应用快照\\prototype\\src\\styles\\prototype.css";
let css = fs.readFileSync(cssPath, "utf8");
css = css.replace(/\r\n/g, "\n");
const start = css.indexOf(".viewport-avatar-center {");
const end = css.indexOf("[data-theme=\"light\"] .cat-stage-orb {");
if (start < 0 || end < 0 || end < start) { console.error("markers not found", start, end); process.exit(1); }
const block = [
".viewport-avatar-center {",
"  position: relative;",
"  z-index: 10;",
"  display: flex;",
"  align-items: center;",
"  justify-content: center;",
"  width: 100%;",
"  height: 100%;",
"}",
"",
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
"",
""
].join("\n");
css = css.slice(0, start) + block + css.slice(end);
fs.writeFileSync(cssPath, css, "utf8");
console.log("done");
