import * as wasm from "test-env";
import { memory } from "test-env/test_env_bg";

const CELL_SIZE = 5;
const CELL_WIDTH = Math.sqrt(3) * CELL_SIZE;
const CELL_HEIGHT = 2 * CELL_SIZE;

const mapRender = new wasm.MapRender();

const run = () => {
  const mapGenRes = mapRender.generateMap(70, 60, 32);

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
      for (let [q, r] of [
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

  const canvas = document.getElementById("mapGenCanvas");

  const bounds = mapRender.bounds();
  const width = bounds[1].x - bounds[0].x;
  const height = bounds[1].y - bounds[0].y;

  canvas.height = CELL_SIZE * height + 2;
  canvas.width = CELL_SIZE * width + 2;
  const ctx = canvas.getContext("2d");

  drawCells(ctx, mapRender);
};

document.getElementById("genMapBtn").onclick = () => run();

run();
