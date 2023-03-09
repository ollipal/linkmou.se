// helper function

const RADIUS = 20;

function degToRad(degrees) {
  const result = (Math.PI / 180) * degrees;
  return result;
}

// setup of the canvas

const canvas = document.querySelector("canvas");
const ctx = canvas.getContext("2d");

let x = 50;
let y = 50;

let x2 = 0;
let y2 = 0;

function canvasDraw() {
  ctx.fillStyle = "black";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#f00";
  ctx.beginPath();
  ctx.arc(x, y, RADIUS, 0, degToRad(360), true);
  ctx.fill();
}
canvasDraw();

let hasBeenLocked = false;

canvas.addEventListener("click", async () => {
  if(!document.pointerLockElement) {
    await canvas.requestPointerLock({
      unadjustedMovement: true,
    });
    hasBeenLocked = true
  }
});

canvas.addEventListener("mouseover", async () => {
  if(!document.pointerLockElement && hasBeenLocked) {
    await canvas.requestPointerLock({
      unadjustedMovement: true,
    });
  }
});


document.addEventListener("pointerlockerror", (event) => {
  console.log("Error locking pointer, requires click again");
  console.log(event);
});


// pointer lock event listeners

document.addEventListener("pointerlockchange", lockChangeAlert, false);

function lockChangeAlert() {
  if (document.pointerLockElement === canvas) {
    console.log("The pointer lock status is now locked");
    x2 = 0;
    y2 = 0;
    document.addEventListener("mousemove", updatePosition, false);
  } else {
    console.log("The pointer lock status is now unlocked");
    document.removeEventListener("mousemove", updatePosition, false);
  }
}

const tracker = document.getElementById("tracker");

let animation;
function updatePosition(e) {
  x += e.movementX;
  y += e.movementY;
  if (x > canvas.width + RADIUS) {
    x = -RADIUS;
  }
  if (y > canvas.height + RADIUS) {
    y = -RADIUS;
  }
  if (x < -RADIUS) {
    x = canvas.width + RADIUS;
  }
  if (y < -RADIUS) {
    y = canvas.height + RADIUS;
  }

  x2 += e.movementX;
  y2 += e.movementY;
  tracker.textContent = `X position: ${x2}, Y position: ${y2}`;

  if (x2 > 0) {
    document.exitPointerLock();
  }

  if (!animation) {
    animation = requestAnimationFrame(function () {
      animation = null;
      canvasDraw();
    });
  }
}
