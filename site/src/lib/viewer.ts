// Draws a unit's mesh (lib/mesh.ts) in WebGL 2, lit as the game's own portraits are
// (crates/mc-models/src/thumbnail.rs `rasterise`): a key light from the upper left of
// the view that casts shadows, sky and bounce light, ambient occlusion, seams between
// armour panels, reflections on metal and glass, and the bloom of its lights. The
// directory's tiles are those portraits, so a tile and this viewer show one look.
//
// Four passes: the key light's shadow map; depth and the lights' glow as the camera
// sees them; the lit model, multisampled; and a last pass that lays occlusion and
// bloom over it and writes the canvas, transparent wherever the model is not.
import { VERTEX, type Mesh, type Vec3 } from "./mesh";

export type View = {
  /** Radians round the model: 0 looks at its nose, positive towards its left side. */
  yaw: number;
  /** Radians above the horizon. */
  pitch: number;
  /** 1 fits the whole model. */
  zoom: number;
};

/** Where the portraits' camera stands (`site.rs` `PORTRAIT_AZIMUTH`, `PORTRAIT_ELEVATION`). */
export const HOME: View = { yaw: (35 * Math.PI) / 180, pitch: (24 * Math.PI) / 180, zoom: 1 };

const FOV = (24 * Math.PI) / 180;
const SHADOW_MAP = 2048;
/** Model z below which the mesh is squashed flat, as a portrait's is (`PORTRAIT_FLOOR`). */
const FLOOR = -4;
const MAX_MATERIALS = 32;

// ---- Small vector and matrix helpers (column-major, as GL takes them) ----

const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const normalize = (a: Vec3): Vec3 => scale(a, 1 / Math.max(Math.hypot(a[0], a[1], a[2]), 1e-9));

type Mat4 = Float32Array;

/** A view matrix from its axes: `x` right, `y` up, `z` back towards the eye. */
function viewOf(eye: Vec3, x: Vec3, y: Vec3, z: Vec3): Mat4 {
  // prettier-ignore
  return new Float32Array([
    x[0], y[0], z[0], 0,
    x[1], y[1], z[1], 0,
    x[2], y[2], z[2], 0,
    -dot(x, eye), -dot(y, eye), -dot(z, eye), 1,
  ]);
}

function perspective(fovY: number, aspect: number, near: number, far: number): Mat4 {
  const f = 1 / Math.tan(fovY / 2);
  // prettier-ignore
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (far + near) / (near - far), -1,
    0, 0, (2 * far * near) / (near - far), 0,
  ]);
}

function ortho(l: number, r: number, b: number, t: number, near: number, far: number): Mat4 {
  // prettier-ignore
  return new Float32Array([
    2 / (r - l), 0, 0, 0,
    0, 2 / (t - b), 0, 0,
    0, 0, -2 / (far - near), 0,
    -(r + l) / (r - l), -(t + b) / (t - b), -(far + near) / (far - near), 1,
  ]);
}

function multiply(a: Mat4, b: Mat4): Mat4 {
  const out = new Float32Array(16);
  for (let c = 0; c < 4; c++) {
    for (let r = 0; r < 4; r++) {
      let s = 0;
      for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
      out[c * 4 + r] = s;
    }
  }
  return out;
}

// ---- Shaders ---------------------------------------------------------------

// Every pass that draws the mesh places a vertex the same way.
const PLACE = /* glsl */ `#version 300 es
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec2 aMat;
layout(location = 2) in vec4 aNormal;
layout(location = 3) in vec4 aFace;
uniform vec3 uLo;
uniform vec3 uSpan;
uniform vec3 uPivot;
uniform vec2 uSpin;
uniform float uFloor;
vec3 place(out vec3 n) {
  vec3 p = uLo + uSpan * aPos;
  n = aNormal.xyz;
  // The byte after the normal is the part: 2 is the spinner, turned about its pivot.
  if (aNormal.w * 127.0 > 1.5) {
    vec2 d = p.xy - uPivot.xy;
    p.xy = uPivot.xy + vec2(d.x * uSpin.x - d.y * uSpin.y, d.x * uSpin.y + d.y * uSpin.x);
    n.xy = vec2(n.x * uSpin.x - n.y * uSpin.y, n.x * uSpin.y + n.y * uSpin.x);
  }
  p.z = max(p.z, uFloor);
  return p;
}
`;

const DEPTH_VS = `${PLACE}
uniform mat4 uMvp;
void main() {
  vec3 n;
  gl_Position = uMvp * vec4(place(n), 1.0);
}`;

const DEPTH_FS = `#version 300 es
precision mediump float;
void main() {}`;

// Depth as the camera sees it, and the light the glowing materials give off.
const GLOW_VS = `${PLACE}
uniform mat4 uMvp;
flat out int vMat;
void main() {
  vec3 n;
  gl_Position = uMvp * vec4(place(n), 1.0);
  vMat = int(aMat.x + 0.5);
}`;

const GLOW_FS = `#version 300 es
precision mediump float;
flat in int vMat;
uniform vec4 uPaint[${MAX_MATERIALS}];
out vec4 oGlow;
void main() {
  vec4 paint = uPaint[vMat];
  oGlow = vec4(mod(paint.a, 2.0) >= 1.0 ? paint.rgb : vec3(0.0), 1.0);
}`;

const SCENE_VS = `${PLACE}
uniform mat4 uMvp;
uniform mat4 uLightMvp;
out vec3 vPos;
out vec3 vNormal;
out vec4 vFace;
out vec3 vShadow;
flat out int vMat;
flat out float vTone;
uniform float uFaceScale;
uniform float uMapTexel;
void main() {
  vec3 n;
  vec3 p = place(n);
  vPos = p;
  vNormal = n;
  vFace = aFace * uFaceScale;
  vMat = int(aMat.x + 0.5);
  // Panels differ a shade from their neighbours, as painted plates do.
  vTone = 1.0 + (aMat.y / 255.0 - 0.5) * 0.09;
  // Looked up a little off the surface, so a face does not shadow itself.
  vec4 s = uLightMvp * vec4(p + n * uMapTexel * 0.8, 1.0);
  vShadow = s.xyz * 0.5 + 0.5;
  gl_Position = uMvp * vec4(p, 1.0);
}`;

// The key light's share that reaches a point, softened over the texels round it.
const LIT = /* glsl */ `
uniform highp sampler2DShadow uShadowMap;
float lit(vec3 at, float bias, float spread) {
  if (at.x <= 0.0 || at.y <= 0.0 || at.x >= 1.0 || at.y >= 1.0) return 1.0;
  float texel = spread / float(${SHADOW_MAP});
  float sum = 0.0;
  for (int y = -1; y <= 1; y++) {
    for (int x = -1; x <= 1; x++) {
      sum += texture(uShadowMap, vec3(at.xy + vec2(x, y) * texel, min(at.z, 1.0) - bias));
    }
  }
  return sum / 9.0;
}
`;

const SCENE_FS = `#version 300 es
precision highp float;
in vec3 vPos;
in vec3 vNormal;
in vec4 vFace;
in vec3 vShadow;
flat in int vMat;
flat in float vTone;
uniform vec4 uPaint[${MAX_MATERIALS}];
uniform vec2 uFinish[${MAX_MATERIALS}];
uniform vec3 uEye;
uniform vec3 uLight;
uniform float uPixel;
uniform vec2 uHeight;
uniform float uDepthTexel;
out vec4 oColor;
${LIT}
// The sky a shiny surface reflects: a bright softbox where the key light is, a cool
// dome and a dark floor.
vec3 sky(vec3 dir) {
  vec3 dome = dir.z > 0.0
    ? mix(vec3(0.62, 0.66, 0.72), vec3(0.3, 0.4, 0.56), sqrt(dir.z))
    : mix(vec3(0.2, 0.19, 0.18), vec3(0.07, 0.07, 0.075), min(-dir.z * 3.0, 1.0));
  return dome + vec3(pow(max(dot(dir, uLight), 0.0), 24.0) * 3.2);
}
// A filmic curve (ACES, fitted), so highlights roll off instead of clipping flat.
vec3 filmic(vec3 x) {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}
void main() {
  const float exposure = 0.62;
  vec4 paint = uPaint[vMat];
  vec3 base = paint.rgb;
  bool emissive = mod(paint.a, 2.0) >= 1.0;
  bool framed = mod(floor(paint.a / 2.0), 2.0) >= 1.0;
  if (emissive) {
    oColor = vec4(filmic(base * 2.2 * exposure), 1.0);
    return;
  }
  vec3 n = normalize(vNormal);
  vec3 v = normalize(uEye - vPos);
  float rough = uFinish[vMat].x;
  float metal = uFinish[vMat].y;
  vec3 albedo = base * vTone;
  // Seams between armour panels, lit on the edge that faces the light.
  if (framed) {
    vec2 st = vFace.xy;
    vec2 size = abs(vFace.zw);
    if (min(size.x, size.y) > uPixel * 2.5) {
      float ex = vFace.z < 0.0 ? 1e9 : size.x - abs(st.x);
      float e = min(ex, size.y - abs(st.y));
      float w = uPixel * 0.75;
      float seam = 1.0 - smoothstep(w * 0.6, w * 1.2, e);
      float lip = (1.0 - smoothstep(w * 2.0, w * 2.6, e)) * (1.0 - seam);
      albedo *= mix(1.0, 0.38, seam) * mix(1.0, 1.12, lip);
      rough = mix(rough, 0.8, seam);
    }
  }
  // Low parts sit a little in the shadow of the rest.
  float height = clamp((vPos.z - uHeight.x) * uHeight.y, 0.0, 1.0);
  float ao = 0.78 + 0.22 * sqrt(height);
  vec3 key = vec3(1.0, 0.95, 0.86) * 2.0;
  float ndl = max(dot(n, uLight), 0.0);
  float shade = ndl > 0.0 ? lit(vShadow, uDepthTexel * (1.5 + 2.5 * (1.0 - ndl)), 1.0) : 0.0;
  float ndv = max(dot(n, v), 1e-3);
  vec3 f0 = mix(vec3(0.04), albedo, metal);
  vec3 fresnel = f0 + (1.0 - f0) * pow(1.0 - ndv, 5.0) * (1.0 - rough * 0.8);
  vec3 hemi = mix(vec3(0.2, 0.19, 0.17), vec3(0.46, 0.5, 0.6), 0.5 + 0.5 * n.z);
  vec3 diffuse = albedo * (1.0 - metal) * (hemi * ao + key * ndl * shade);
  vec3 h = normalize(uLight + v);
  float power = 2.0 / max(rough * rough * rough * rough, 1e-4) - 2.0;
  vec3 spec = key * shade * ndl * pow(max(dot(n, h), 0.0), power) * (power + 8.0) / 25.0;
  vec3 env = sky(reflect(-v, n)) * pow(1.0 - rough, 1.5) * ao;
  // A cool rim along the far edges, to lift the outline off a dark page.
  vec3 rim = vec3(0.5, 0.6, 0.75) * pow(1.0 - ndv, 3.0) * 0.35 * ao;
  oColor = vec4(filmic((diffuse + fresnel * (spec + env) + rim) * exposure), 1.0);
}`;

// The shadow the model throws on the ground under it.
const GROUND_VS = `#version 300 es
layout(location = 0) in vec2 aCorner;
uniform mat4 uMvp;
uniform mat4 uLightMvp;
uniform vec3 uCentre;
uniform float uReach;
out vec3 vShadow;
out vec2 vOff;
void main() {
  vec3 p = uCentre + vec3(aCorner * uReach, 0.0);
  vec4 s = uLightMvp * vec4(p, 1.0);
  vShadow = s.xyz * 0.5 + 0.5;
  vOff = aCorner;
  gl_Position = uMvp * vec4(p, 1.0);
}`;

const GROUND_FS = `#version 300 es
precision highp float;
in vec3 vShadow;
in vec2 vOff;
out vec4 oColor;
${LIT}
void main() {
  float shade = 1.0 - lit(vShadow, 0.0, 3.0);
  float fade = 1.0 - smoothstep(0.55, 1.0, length(vOff));
  oColor = vec4(0.0, 0.0, 0.0, shade * 0.5 * fade);
}`;

const POST_VS = `#version 300 es
out vec2 vUv;
void main() {
  vec2 p = vec2(gl_VertexID == 1 ? 3.0 : -1.0, gl_VertexID == 2 ? 3.0 : -1.0);
  vUv = p * 0.5 + 0.5;
  gl_Position = vec4(p, 0.0, 1.0);
}`;

// Occlusion from the depth the camera saw: how far the surroundings rise toward it,
// looked for in eight directions at two reaches. Then the bloom of the model's lights,
// spilling past its edges.
const POST_FS = `#version 300 es
precision highp float;
in vec2 vUv;
uniform sampler2D uColor;
uniform sampler2D uDepth;
uniform sampler2D uGlow;
uniform vec2 uNearFar;
uniform vec2 uReach;
uniform float uWorldReach;
uniform float uBloomLod;
out vec4 oColor;
float distanceAt(vec2 uv) {
  float d = texture(uDepth, uv).r;
  if (d >= 1.0) return -1.0;
  float ndc = d * 2.0 - 1.0;
  return 2.0 * uNearFar.x * uNearFar.y / (uNearFar.y + uNearFar.x - ndc * (uNearFar.y - uNearFar.x));
}
vec3 encode(vec3 c) {
  return mix(c * 12.92, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(0.0031308, c));
}
void main() {
  vec4 c = texture(uColor, vUv);
  float here = distanceAt(vUv);
  if (here > 0.0) {
    float open = 0.0;
    for (int k = 0; k < 8; k++) {
      float a = float(k) * 0.7853982 + 0.4;
      vec2 dir = vec2(cos(a), sin(a));
      float worst = 0.0;
      for (int s = 0; s < 2; s++) {
        float reach = s == 0 ? 0.45 : 1.0;
        float there = distanceAt(vUv + dir * uReach * reach);
        if (there < 0.0) continue;
        float rise = (here - there) / (uWorldReach * reach);
        // Steep drops away do not open a hole, and far rises fade.
        float falloff = clamp(1.0 - (here - there) / (uWorldReach * 3.0), 0.0, 1.0);
        worst = max(worst, max(clamp(rise, 0.0, 1.5) - 0.12, 0.0) * falloff);
      }
      open += 1.0 - min(worst, 1.0);
    }
    c.rgb *= mix(1.0, pow(open / 8.0, 1.4), 0.85);
  }
  vec3 glow = (textureLod(uGlow, vUv, uBloomLod).rgb + textureLod(uGlow, vUv, uBloomLod + 1.0).rgb
    + textureLod(uGlow, vUv, uBloomLod + 2.0).rgb) * 0.42;
  float covered = here > 0.0 ? 1.0 : 0.0;
  vec3 rgb = c.rgb + glow * mix(1.0, 0.35, covered);
  float alpha = max(c.a, min(max(glow.r, max(glow.g, glow.b)) * 1.6, 1.0));
  // The canvas takes sRGB, premultiplied.
  vec3 straight = alpha > 0.0 ? min(rgb / alpha, 1.0) : vec3(0.0);
  oColor = vec4(encode(straight) * alpha, alpha);
}`;

function compile(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const program = gl.createProgram();
  for (const [type, source] of [
    [gl.VERTEX_SHADER, vs],
    [gl.FRAGMENT_SHADER, fs],
  ] as const) {
    const shader = gl.createShader(type)!;
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS))
      throw new Error(gl.getShaderInfoLog(shader) ?? "a shader did not compile");
    gl.attachShader(program, shader);
    gl.deleteShader(shader);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS))
    throw new Error(gl.getProgramInfoLog(program) ?? "a program did not link");
  return program;
}

/** The render targets of one canvas size. */
type Targets = {
  width: number;
  height: number;
  /** The lit model, multisampled, and what it resolves into. */
  scene: WebGLFramebuffer;
  sceneColor: WebGLRenderbuffer;
  sceneDepth: WebGLRenderbuffer;
  resolve: WebGLFramebuffer;
  color: WebGLTexture;
  /** Depth and glow as the camera sees them. */
  pre: WebGLFramebuffer;
  depth: WebGLTexture;
  glow: WebGLTexture;
};

export class UnitRenderer {
  private gl: WebGL2RenderingContext;
  private mesh: Mesh;
  private programs: { depth: WebGLProgram; glow: WebGLProgram; scene: WebGLProgram; ground: WebGLProgram; post: WebGLProgram };
  private uniforms = new Map<WebGLProgram, Map<string, WebGLUniformLocation | null>>();
  private meshVao: WebGLVertexArrayObject;
  private groundVao: WebGLVertexArrayObject;
  private buffers: WebGLBuffer[] = [];
  private shadow: WebGLFramebuffer;
  private shadowMap: WebGLTexture;
  private targets: Targets | null = null;
  private paint: Float32Array;
  private finish: Float32Array;
  /** The bounds as drawn (squashed at the floor), their middle and the sphere round them. */
  private lo: Vec3;
  private centre: Vec3;
  private radius: number;
  /** Whether anything in the mesh moves by itself, so the picture changes with time. */
  readonly moves: boolean;

  constructor(canvas: HTMLCanvasElement, mesh: Mesh) {
    const gl = canvas.getContext("webgl2", {
      alpha: true,
      premultipliedAlpha: true,
      antialias: false,
      powerPreference: "high-performance",
    });
    if (!gl) throw new Error("no WebGL 2");
    this.gl = gl;
    this.mesh = mesh;
    this.programs = {
      depth: compile(gl, DEPTH_VS, DEPTH_FS),
      glow: compile(gl, GLOW_VS, GLOW_FS),
      scene: compile(gl, SCENE_VS, SCENE_FS),
      ground: compile(gl, GROUND_VS, GROUND_FS),
      post: compile(gl, POST_VS, POST_FS),
    };

    this.lo = [mesh.lo[0], mesh.lo[1], Math.max(mesh.lo[2], FLOOR)];
    this.centre = scale(add(this.lo, mesh.hi), 0.5);
    this.radius = Math.hypot(...sub(mesh.hi, this.lo)) / 2;

    // The paint: colour and flag bits, roughness and metal, by material.
    this.paint = new Float32Array(MAX_MATERIALS * 4);
    this.finish = new Float32Array(MAX_MATERIALS * 2);
    mesh.materials.slice(0, MAX_MATERIALS).forEach((m, i) => {
      this.paint.set([...m.color, (m.emissive ? 1 : 0) + (m.framed ? 2 : 0)], i * 4);
      this.finish.set([m.rough, m.metal], i * 2);
    });

    // The mesh, read straight out of the file's vertices.
    this.meshVao = gl.createVertexArray();
    gl.bindVertexArray(this.meshVao);
    const vertices = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, vertices);
    gl.bufferData(gl.ARRAY_BUFFER, mesh.vertices, gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 3, gl.UNSIGNED_SHORT, true, VERTEX, 0);
    gl.enableVertexAttribArray(1);
    gl.vertexAttribPointer(1, 2, gl.UNSIGNED_BYTE, false, VERTEX, 6);
    gl.enableVertexAttribArray(2);
    gl.vertexAttribPointer(2, 4, gl.BYTE, true, VERTEX, 8);
    gl.enableVertexAttribArray(3);
    gl.vertexAttribPointer(3, 4, gl.SHORT, false, VERTEX, 12);
    const indices = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indices);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.indices, gl.STATIC_DRAW);

    this.groundVao = gl.createVertexArray();
    gl.bindVertexArray(this.groundVao);
    const corners = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, corners);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    this.buffers = [vertices, indices, corners];

    // The key light's shadow map, compared and filtered by the sampler.
    this.shadowMap = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, this.shadowMap);
    gl.texStorage2D(gl.TEXTURE_2D, 1, gl.DEPTH_COMPONENT24, SHADOW_MAP, SHADOW_MAP);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_COMPARE_MODE, gl.COMPARE_REF_TO_TEXTURE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_COMPARE_FUNC, gl.LEQUAL);
    this.shadow = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadow);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, this.shadowMap, 0);
    gl.drawBuffers([gl.NONE]);
    gl.readBuffer(gl.NONE);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    let moves = false;
    for (let i = 0; i < mesh.vertexCount && !moves; i++) moves = mesh.vertices[i * VERTEX + 11] === 2;
    this.moves = moves;
  }

  /** Repaints the owner's colour: `team` over what the faction's own was (both linear RGB). */
  setTeam(team: Vec3, was: Vec3) {
    this.mesh.materials.slice(0, MAX_MATERIALS).forEach((m, i) => {
      // The file's colour is the paint the game makes of the owner's: the same share of the new one.
      if (m.team)
        this.paint.set(
          m.color.map((c, k) => (c / Math.max(was[k], 1e-3)) * team[k]),
          i * 4,
        );
    });
  }

  private uniform(program: WebGLProgram, name: string) {
    let of = this.uniforms.get(program);
    if (!of) this.uniforms.set(program, (of = new Map()));
    if (!of.has(name)) of.set(name, this.gl.getUniformLocation(program, name));
    return of.get(name)!;
  }

  private resize(width: number, height: number) {
    const gl = this.gl;
    if (this.targets?.width === width && this.targets.height === height) return this.targets;
    this.dropTargets();
    const samples = Math.min(4, gl.getParameter(gl.MAX_SAMPLES) as number);

    const sceneColor = gl.createRenderbuffer();
    gl.bindRenderbuffer(gl.RENDERBUFFER, sceneColor);
    gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.SRGB8_ALPHA8, width, height);
    const sceneDepth = gl.createRenderbuffer();
    gl.bindRenderbuffer(gl.RENDERBUFFER, sceneDepth);
    gl.renderbufferStorageMultisample(gl.RENDERBUFFER, samples, gl.DEPTH_COMPONENT24, width, height);
    const scene = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, scene);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.RENDERBUFFER, sceneColor);
    gl.framebufferRenderbuffer(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.RENDERBUFFER, sceneDepth);

    const texture = (format: number, levels: number, filter: number) => {
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texStorage2D(gl.TEXTURE_2D, levels, format, width, height);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, filter);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, filter === gl.NEAREST ? gl.NEAREST : gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      return t;
    };
    const color = texture(gl.SRGB8_ALPHA8, 1, gl.LINEAR);
    const resolve = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, resolve);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, color, 0);

    const levels = Math.floor(Math.log2(Math.max(width, height))) + 1;
    const glow = texture(gl.RGBA8, levels, gl.LINEAR_MIPMAP_LINEAR);
    const depth = texture(gl.DEPTH_COMPONENT24, 1, gl.NEAREST);
    const pre = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, pre);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, glow, 0);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.DEPTH_ATTACHMENT, gl.TEXTURE_2D, depth, 0);
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);

    this.targets = { width, height, scene, sceneColor, sceneDepth, resolve, color, pre, depth, glow };
    return this.targets;
  }

  private dropTargets() {
    const gl = this.gl;
    const t = this.targets;
    if (!t) return;
    for (const f of [t.scene, t.resolve, t.pre]) gl.deleteFramebuffer(f);
    for (const r of [t.sceneColor, t.sceneDepth]) gl.deleteRenderbuffer(r);
    for (const x of [t.color, t.depth, t.glow]) gl.deleteTexture(x);
    this.targets = null;
  }

  /** Draws the model from `view` into a canvas `width` by `height` pixels, `time` seconds in. */
  render(view: View, width: number, height: number, time: number) {
    const gl = this.gl;
    const mesh = this.mesh;
    const t = this.resize(width, height);

    // The camera, turning about the model's middle.
    const toEye: Vec3 = [
      Math.cos(view.pitch) * Math.cos(view.yaw),
      Math.cos(view.pitch) * Math.sin(view.yaw),
      Math.sin(view.pitch),
    ];
    const right = normalize(cross(scale(toEye, -1), [0, 0, 1]));
    const up = cross(right, scale(toEye, -1));
    const aspect = width / height;
    // The sphere round the model in sight, across the narrower way of the canvas.
    // A box's corners are rarely all filled, so the sphere round it is let run a little over.
    const fit = (this.radius * 0.9) / Math.sin(FOV / 2) / Math.min(aspect, 1);
    const distance = fit / view.zoom;
    const eye = add(this.centre, scale(toEye, distance));
    const near = Math.max(distance - this.radius * 3, distance * 0.02);
    const far = distance + this.radius * 3;
    const mvp = multiply(perspective(FOV, aspect, near, far), viewOf(eye, right, up, toEye));
    /** Metres one pixel covers at the model's middle. */
    const pixel = (2 * distance * Math.tan(FOV / 2)) / height;

    // The key light from the upper left of the view and a little behind, so the shadow
    // falls toward the viewer's lower right and the tops catch it.
    const light = normalize(add(add(add([0, 0, 0.8], scale(right, -0.62)), scale(toEye, -0.12)), scale(up, 0.1)));
    const lx = normalize(cross(light, [0, 0, 1]));
    const ly = cross(lx, light);
    const reach = this.radius * 1.05;
    const mapTexel = (2 * reach) / SHADOW_MAP;
    const lightMvp = multiply(
      ortho(-reach, reach, -reach, reach, 0, 2 * reach),
      viewOf(add(this.centre, scale(light, reach)), lx, ly, light),
    );

    // The spinner: round and round, or a slow look about (`entity.wgsl`).
    const spin = mesh.spinnerScans ? (1.4 * (Math.sin(time * 0.23) + 0.35 * Math.sin(time * 0.61))) / 1.35 : time * 1.6;

    const place = (program: WebGLProgram) => {
      gl.useProgram(program);
      gl.uniform3fv(this.uniform(program, "uLo"), mesh.lo);
      gl.uniform3fv(this.uniform(program, "uSpan"), sub(mesh.hi, mesh.lo));
      gl.uniform3fv(this.uniform(program, "uPivot"), mesh.spinnerPivot);
      gl.uniform2f(this.uniform(program, "uSpin"), Math.cos(spin), Math.sin(spin));
      gl.uniform1f(this.uniform(program, "uFloor"), FLOOR);
    };
    const drawMesh = () => {
      gl.bindVertexArray(this.meshVao);
      gl.drawElements(gl.TRIANGLES, mesh.indexCount, gl.UNSIGNED_INT, 0);
    };

    gl.enable(gl.DEPTH_TEST);
    gl.depthFunc(gl.LEQUAL);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.clearDepth(1);

    // 1. The shadow map: every face, either way round.
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.shadow);
    gl.viewport(0, 0, SHADOW_MAP, SHADOW_MAP);
    gl.clear(gl.DEPTH_BUFFER_BIT);
    gl.disable(gl.CULL_FACE);
    place(this.programs.depth);
    gl.uniformMatrix4fv(this.uniform(this.programs.depth, "uMvp"), false, lightMvp);
    drawMesh();

    // 2. Depth and glow as the camera sees them. Back faces are culled, as in a portrait.
    gl.enable(gl.CULL_FACE);
    gl.cullFace(gl.BACK);
    gl.bindFramebuffer(gl.FRAMEBUFFER, t.pre);
    gl.viewport(0, 0, width, height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    place(this.programs.glow);
    gl.uniformMatrix4fv(this.uniform(this.programs.glow, "uMvp"), false, mvp);
    gl.uniform4fv(this.uniform(this.programs.glow, "uPaint"), this.paint);
    drawMesh();
    gl.bindTexture(gl.TEXTURE_2D, t.glow);
    gl.generateMipmap(gl.TEXTURE_2D);

    // 3. The lit model over its shadow on the ground.
    gl.bindFramebuffer(gl.FRAMEBUFFER, t.scene);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.shadowMap);

    const ground = this.programs.ground;
    gl.useProgram(ground);
    gl.uniformMatrix4fv(this.uniform(ground, "uMvp"), false, mvp);
    gl.uniformMatrix4fv(this.uniform(ground, "uLightMvp"), false, lightMvp);
    gl.uniform3f(this.uniform(ground, "uCentre"), this.centre[0], this.centre[1], Math.min(this.lo[2], 0));
    gl.uniform1f(this.uniform(ground, "uReach"), this.radius * 2.2);
    gl.uniform1i(this.uniform(ground, "uShadowMap"), 0);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    gl.depthMask(false);
    gl.disable(gl.CULL_FACE);
    gl.bindVertexArray(this.groundVao);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    gl.depthMask(true);
    gl.disable(gl.BLEND);
    gl.enable(gl.CULL_FACE);

    const scene = this.programs.scene;
    place(scene);
    gl.uniformMatrix4fv(this.uniform(scene, "uMvp"), false, mvp);
    gl.uniformMatrix4fv(this.uniform(scene, "uLightMvp"), false, lightMvp);
    gl.uniform1f(this.uniform(scene, "uFaceScale"), mesh.faceScale);
    gl.uniform1f(this.uniform(scene, "uMapTexel"), mapTexel);
    gl.uniform1f(this.uniform(scene, "uDepthTexel"), mapTexel / (2 * reach));
    gl.uniform4fv(this.uniform(scene, "uPaint"), this.paint);
    gl.uniform2fv(this.uniform(scene, "uFinish"), this.finish);
    gl.uniform3fv(this.uniform(scene, "uEye"), eye);
    gl.uniform3fv(this.uniform(scene, "uLight"), light);
    gl.uniform1f(this.uniform(scene, "uPixel"), pixel);
    gl.uniform2f(this.uniform(scene, "uHeight"), this.lo[2], 1 / Math.max(mesh.hi[2] - this.lo[2], 1e-3));
    gl.uniform1i(this.uniform(scene, "uShadowMap"), 0);
    drawMesh();

    gl.bindFramebuffer(gl.READ_FRAMEBUFFER, t.scene);
    gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, t.resolve);
    gl.blitFramebuffer(0, 0, width, height, 0, 0, width, height, gl.COLOR_BUFFER_BIT, gl.NEAREST);

    // 4. Onto the canvas, with occlusion and bloom.
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.viewport(0, 0, width, height);
    gl.disable(gl.DEPTH_TEST);
    gl.disable(gl.CULL_FACE);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    const post = this.programs.post;
    gl.useProgram(post);
    [t.color, t.depth, t.glow].forEach((texture, unit) => {
      gl.activeTexture(gl.TEXTURE0 + unit);
      gl.bindTexture(gl.TEXTURE_2D, texture);
    });
    gl.uniform1i(this.uniform(post, "uColor"), 0);
    gl.uniform1i(this.uniform(post, "uDepth"), 1);
    gl.uniform1i(this.uniform(post, "uGlow"), 2);
    gl.uniform2f(this.uniform(post, "uNearFar"), near, far);
    const reachPx = Math.max(Math.min(width, height) / 34, 3);
    gl.uniform2f(this.uniform(post, "uReach"), reachPx / width, reachPx / height);
    gl.uniform1f(this.uniform(post, "uWorldReach"), reachPx * pixel);
    // The bloom is about a twenty-eighth of the picture wide.
    gl.uniform1f(this.uniform(post, "uBloomLod"), Math.max(Math.log2(Math.min(width, height) / 28) - 1.5, 0));
    gl.bindVertexArray(null);
    gl.drawArrays(gl.TRIANGLES, 0, 3);
    gl.activeTexture(gl.TEXTURE0);
  }

  dispose() {
    const gl = this.gl;
    this.dropTargets();
    gl.deleteFramebuffer(this.shadow);
    gl.deleteTexture(this.shadowMap);
    gl.deleteVertexArray(this.meshVao);
    gl.deleteVertexArray(this.groundVao);
    for (const b of this.buffers) gl.deleteBuffer(b);
    for (const p of Object.values(this.programs)) gl.deleteProgram(p);
  }
}
