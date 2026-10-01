// A unit's mesh as the game's exporter writes it (crates/mc-models/src/site.rs,
// `encode`): the model at rest, with its own paint, so nothing here knows a material
// by number. The file is gzipped by scripts/models.mjs and opened here.

export type Vec3 = [number, number, number];

export type Material = {
  /** Linear RGB. */
  color: Vec3;
  emissive: boolean;
  /** Armour: seams run round each face's frame. */
  framed: boolean;
  /** The owner's colour: the viewer may repaint it. */
  team: boolean;
  rough: number;
  metal: number;
};

export type Mesh = {
  vertexCount: number;
  indexCount: number;
  /** The bounds, metres, model space: x forward, y left, z up, the origin on the ground under the middle. */
  lo: Vec3;
  hi: Vec3;
  /** Metres to one unit of a face frame's i16s. */
  faceScale: number;
  spinnerPivot: Vec3;
  /** The spinner looks about rather than turning round. */
  spinnerScans: boolean;
  materials: Material[];
  /** `VERTEX` bytes each: see `encode` for the layout. */
  vertices: Uint8Array;
  indices: Uint32Array;
};

const MAGIC = 0x324d434d; // "MCM2", little-endian
const HEADER = 72;
const MATERIAL = 16;
export const VERTEX = 20;

const FLAG_EMISSIVE = 1;
const FLAG_FRAMED = 2;
const FLAG_TEAM = 4;

export function parseMesh(buffer: ArrayBuffer): Mesh {
  const view = new DataView(buffer);
  if (buffer.byteLength < HEADER || view.getUint32(0, true) !== MAGIC) throw new Error("not a mesh file");
  const vertexCount = view.getUint32(4, true);
  const indexCount = view.getUint32(8, true);
  const materialCount = view.getUint32(12, true);
  const firstVertex = HEADER + materialCount * MATERIAL;
  const firstIndex = firstVertex + vertexCount * VERTEX;
  if (buffer.byteLength !== firstIndex + indexCount * 4) throw new Error("a mesh file of the wrong length");
  const f = (at: number) => view.getFloat32(at, true);
  const vec = (at: number): Vec3 => [f(at), f(at + 4), f(at + 8)];
  const materials: Material[] = [];
  for (let i = 0; i < materialCount; i++) {
    const at = HEADER + i * MATERIAL;
    const flags = view.getUint8(at + 12);
    materials.push({
      color: vec(at),
      emissive: (flags & FLAG_EMISSIVE) !== 0,
      framed: (flags & FLAG_FRAMED) !== 0,
      team: (flags & FLAG_TEAM) !== 0,
      rough: view.getUint8(at + 13) / 255,
      metal: view.getUint8(at + 14) / 255,
    });
  }
  return {
    vertexCount,
    indexCount,
    lo: vec(16),
    hi: vec(28),
    faceScale: f(40),
    spinnerPivot: vec(56),
    spinnerScans: view.getUint32(68, true) === 1,
    materials,
    vertices: new Uint8Array(buffer, firstVertex, vertexCount * VERTEX),
    indices: new Uint32Array(buffer, firstIndex, indexCount),
  };
}

export async function loadMesh(url: string, signal?: AbortSignal): Promise<Mesh> {
  const res = await fetch(url, { signal });
  if (!res.ok || !res.body) throw new Error(`${url}: ${res.status}`);
  const opened = res.body.pipeThrough(new DecompressionStream("gzip"));
  return parseMesh(await new Response(opened).arrayBuffer());
}
