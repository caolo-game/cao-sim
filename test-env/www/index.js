import * as wasm from "test-env";
import { memory } from "test-env/test_env_bg";

const CELL_SIZE = 3;
const CELL_WIDTH = Math.sqrt(3) * CELL_SIZE;
const CELL_HEIGHT = 2 * CELL_SIZE;

const mapRender = new wasm.MapRender();

var count = 0;
var running = false;

var plainDilation = 1;
var chancePlain = 1.0 / 3.0;
var chanceWall = 1.0 / 3.0;
var seed = null;
var colorBridges = false;
var mapRadius = 4;
var roomRadius = 16;

const render = () => {
  console.time("render");
  const canvas = document.getElementById("mapGenCanvas");
  const ctx = canvas.getContext("2d");
  ctx.clearRect(0, 0, canvas.width, canvas.height);

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
        case "Bridge":
          if (colorBridges) {
            ctx.fillStyle = "#89a13a";
            break;
          }
        // else fall through
        case "Plain":
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

  canvas.height = CELL_SIZE * (height + 1);
  canvas.width = CELL_SIZE * (width + 2);

  drawCells(ctx, mapRender);

  console.timeEnd("render");
};

const _run = () => {
  console.log("================ run ================");

  let error = null;
  console.time("running mapgen");
  try {
    mapRender.generateMap(
      mapRadius,
      roomRadius,
      chancePlain,
      chanceWall,
      plainDilation,
      seed
    );
  } catch (e) {
    error = e;
  } finally {
    console.timeEnd("running mapgen");
  }

  render();

  if (error) {
    throw error;
  }
};

const runOnce = () => {
  count += 1;
  console.log("seed", seed);
  try {
    _run();
  } catch (e) {
    console.error("Failed to run", e);
    throw e;
  } finally {
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

document.getElementById("plainChance").value = Math.floor(chancePlain * 100);
document.getElementById("wallChance").value = Math.floor(chanceWall * 100);
document.getElementById("plainDilation").value = plainDilation;
document.getElementById("mapRadius").value = mapRadius;
document.getElementById("roomRadius").value = roomRadius;

document.getElementById("plainChance").onchange = (el) => {
  chancePlain = parseFloat(el.target.value) / 100.0;
};

document.getElementById("wallChance").onchange = (el) => {
  chanceWall = parseFloat(el.target.value) / 100.0;
};

document.getElementById("plainDilation").onchange = (el) => {
  plainDilation = parseInt(el.target.value);
};

document.getElementById("seed").onchange = (el) => {
  seed = el.target.value;
};

document.getElementById("renderBridge").onchange = (el) => {
  colorBridges = el.target.checked;
  render();
};
document.getElementById("renderBridge").on = colorBridges;

document.getElementById("mapRadius").onchange = (el) => {
  mapRadius = parseInt(el.target.value);
  render();
};

document.getElementById("roomRadius").onchange = (el) => {
  roomRadius = parseInt(el.target.value);
  render();
};

runOnce();
