const fs = require("fs");
const p = "C:\\Users\\32875\\Desktop\\应用快照\\prototype\\src\\styles\\prototype.css";
let css = fs.readFileSync(p, "utf8");
css = css.replace(/\r\n/g, "\n");

// 删除 pedestal / aura-ring / stage-theme 相关规则块
css = css.replace(/\.stage-aura-ring \{[^}]*\}\s*/g, "");
css = css.replace(/\.stage-aura-ring\.is-disabled \{[^}]*\}\s*/g, "");
css = css.replace(/\.stage-aura-ring\.is-reacting \{[^}]*\}\s*/g, "");
css = css.replace(/\.cat-stage-pedestal \{[^}]*\}\s*/g, "");
css = css.replace(/\[data-theme="light"\] \.stage-aura-ring \{[^}]*\}\s*/g, "");
css = css.replace(/\[data-theme="light"\] \.cat-stage-pedestal \{[^}]*\}\s*/g, "");
css = css.replace(/\.viewport-desktop-canvas\[data-stage-theme[^}]*\}\s*/g, "");

fs.writeFileSync(p, css, "utf8");
console.log("cleaned");
