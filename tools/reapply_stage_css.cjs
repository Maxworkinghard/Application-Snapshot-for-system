const fs = require("fs");
const p = "C:\\Users\\32875\\Desktop\\应用快照\\prototype\\src\\styles\\prototype.css";
let css = fs.readFileSync(p, "utf8");
css = css.replace(/\r\n/g, "\n");

css = css.replace(/\.viewport-desktop-canvas \{[^}]*background: var\(--bg-input\);[^}]*\}/, [
".viewport-desktop-canvas {",
"  position: relative;",
"  height: 220px;",
"  border-radius: var(--radius-md);",
"  overflow: hidden;",
"  display: flex;",
"  flex-direction: column;",
"  align-items: center;",
"  justify-content: center;",
"  background: #ffffff;",
"  border: 1px solid var(--border-subtle);",
"}",
].join("\n"));

fs.writeFileSync(p, css, "utf8");
console.log("done");
