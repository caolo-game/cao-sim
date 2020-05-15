import * as wasm from "test-env";
import { memory } from "test-env/test_env_bg";

const CELL_SIZE = 10;

const mapRender = new wasm.MapRender();
const mapGenRes = mapRender.generateMap(64, 64, 32);

document.getElementById("mapGenRes").innerHTML = `<pre>${mapGenRes}</pre>`;

const drawCells = (ctx, mapRender) => {
  const bounds = mapRender.bounds();
  const { x: offsetx, y: offsety } = bounds[0];

  const cells = mapRender.getCells();

  console.log(cells);

  ctx.beginPath();

  for (let cell of cells) {
    switch (cell[1]) {
      case "Plain":
        ctx.fillStyle = "#898130";
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
    ctx.fillRect(
      x * (CELL_SIZE + 1) + 1,
      y * (CELL_SIZE + 1) + 1,
      CELL_SIZE,
      CELL_SIZE
    );
  }

  ctx.stroke();
};

const canvas = document.getElementById("mapGenCanvas");

const bounds = mapRender.bounds();
const width = bounds[1].x - bounds[0].x;
const height = bounds[1].y - bounds[0].y;

console.log(bounds);

canvas.height = (CELL_SIZE + 1) * height + 1;
canvas.width = (CELL_SIZE + 1) * width + 1;
const ctx = canvas.getContext("2d");

drawCells(ctx, mapRender);
