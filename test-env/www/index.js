import * as wasm from "test-env";
import { memory } from "test-env/test_env_bg";

const CELL_SIZE = 5;
const CELL_WIDTH = Math.sqrt(3) * CELL_SIZE;
const CELL_HEIGHT = 2 * CELL_SIZE;

const mapRender = new wasm.MapRender();

var COUNT = 0;
var running = true;

const _run = () => {
  const canvas = document.getElementById("mapGenCanvas");
  const ctx = canvas.getContext("2d");
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  let error = null;
  let mapGenRes = null;
  try {
    mapGenRes = mapRender.generateMap(32);
  } catch (e) {
    error = e;
  }

  document.getElementById("mapGenRes").innerHTML = `<pre>${mapGenRes}</pre>`;

  const drawCells = (ctx, mapRender) => {
    const bounds = mapRender.bounds();
    const { x: offsetx, y: offsety } = bounds[0];

    const cells = mapRender.getCells();

    console.log("cells", cells);
    console.log("bounds", bounds);

    console.log("drawing");

    for (let cell of cells) {
      switch (cell[1]) {
        case "Plain":
        case "Edge":
          ctx.fillStyle = "#89813a";
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

  canvas.height = CELL_SIZE * height + 2;
  canvas.width = CELL_SIZE * width + 2;

  drawCells(ctx, mapRender);

  if (error) {
    throw error;
  }
};

const runOnce = () => {
  COUNT += 1;
  console.time("running");
  try {
    _run();
  } catch (e) {
    console.error("Failed to run", e);
    throw e;
  } finally {
    console.timeEnd("running");
    console.log("Run ", COUNT, "done");
  }
};

const run = () => {
  if (!running) return;
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

run();
