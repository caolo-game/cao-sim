import * as wasm from "test-env";
import { memory } from "test-env/test_env_bg";

const CELL_SIZE = 3;
const CELL_WIDTH = Math.sqrt(3) * CELL_SIZE;
const CELL_HEIGHT = 2 * CELL_SIZE;

const mapRender = new wasm.MapRender();

var count = 0;
var running = false;

var plain_dilation = 1;
var chance_plain = 1.0 / 3.0;
var chance_wall = 1.0 / 3.0;
var seed = null;

const _run = () => {
  console.log("================ run ================");

  const canvas = document.getElementById("mapGenCanvas");
  const ctx = canvas.getContext("2d");
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  let error = null;
  console.time("running mapgen");
  try {
    mapRender.generateMap(16, chance_plain, chance_wall, plain_dilation, seed);
  } catch (e) {
    error = e;
  } finally {
    console.timeEnd("running mapgen");
  }

  const drawCells = (ctx, mapRender) => {
    const bounds = mapRender.bounds();
    let { x: offsetx, y: offsety } = bounds[0];
    offsety -= 1;

    const cells = mapRender.getCells();

    console.debug("cells", cells);
    console.debug("bounds", bounds);

    console.debug("drawing");

    for (let cell of cells) {
      switch (cell[1]) {
        case "Plain":
          ctx.fillStyle = "#89813a";
          break;
        case "Bridge":
          ctx.fillStyle = "#89a13a";
          break;
        case "Wall":
          ctx.fillStyle = "#B3AD6A";
          break;

        default:
          throw `Unknown tile type: ${cell}`;
      }
      let { x, y } = cell[0];
      x -= offsetx;
      y -= offsety;
      x *= CELL_SIZE;
      y *= CELL_SIZE;

      ctx.beginPath();
      ctx.moveTo(x, y);
      for (const [q, r] of [
        // [0, 0],
        [CELL_WIDTH / 2, CELL_HEIGHT / 4],
        [CELL_WIDTH, 0],
        [CELL_WIDTH, -CELL_HEIGHT / 2],
        [CELL_WIDTH / 2, (-CELL_HEIGHT * 3) / 4],
        [0, -CELL_HEIGHT / 2],
      ]) {
        ctx.lineTo(x + q, y + r);
      }
      ctx.closePath();
      ctx.fill();
    }

    console.log("drawing done");
  };

  const bounds = mapRender.bounds();
  const width = bounds[1].x - bounds[0].x;
  const height = bounds[1].y - bounds[0].y;

  canvas.height = CELL_SIZE * (height + 1);
  canvas.width = CELL_SIZE * (width + 2);

  drawCells(ctx, mapRender);

  if (error) {
    throw error;
  }
};

const runOnce = () => {
  count += 1;
  console.time("running");
  console.log("seed", seed);
  try {
    _run();
  } catch (e) {
    console.error("Failed to run", e);
    throw e;
  } finally {
    console.timeEnd("running");
    console.log("Run ", count, "done");
  }
};

const run = () => {
  if (!running) return;
  const s = "ASDFGHJKLMNBVCXZQWERTYUIOPasdfghjklmnbvcxzqwertyuiop09876543210";

  seed = Array.apply(null, Array(16))
    .map(function () {
      return s.charAt(Math.floor(Math.random() * s.length));
    })
    .join("");
  document.getElementById("seed").value = seed;
  runOnce();
  setTimeout(run, 1000);
};

document.getElementById("genMapToggle").onclick = () => {
  running = !running;
  run();
};

document.getElementById("genMapBtn").onclick = () => {
  runOnce();
};

document.getElementById("plain_chance").value = Math.floor(chance_plain * 100);
document.getElementById("wall_chance").value = Math.floor(chance_wall * 100);
document.getElementById("plain_dilation").value = plain_dilation;

document.getElementById("plain_chance").onchange = (el) => {
  chance_plain = parseFloat(el.target.value) / 100.0;
};

document.getElementById("wall_chance").onchange = (el) => {
  chance_wall = parseFloat(el.target.value) / 100.0;
};

document.getElementById("plain_dilation").onchange = (el) => {
  plain_dilation = parseInt(el.target.value);
};

document.getElementById("seed").onchange = (el) => {
  seed = el.target.value;
};

runOnce();
