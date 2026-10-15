import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'
import { compileScript, parse } from '@vue/compiler-sfc'
import ts from 'typescript'
import * as vue from 'vue'
import { renderToString } from '@vue/server-renderer'


const { descriptor } = parse(readFileSync(new URL('./components/Board.vue', import.meta.url), 'utf8'))
const compiled = ts.transpileModule(compileScript(descriptor, { id: 'occupation-board', inlineTemplate: true }).content, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2020 },
}).outputText
const exports: Record<string, unknown> = {}
new Function('require', 'exports', compiled)((id: string) => {
  if (id === 'vue') return vue
  if (id === '../presentationEvents') return { boardOccupancy: () => new Map(), presentationEvents: () => [] }
  if (id === '../presentationSound') return { presentationSound: { play() {} } }
  if (id === '../moveOptionUi') return { activeCooldownRemaining: () => 0 }
  if (id === '../pieceAssets') return { renderedPieceAsset: () => undefined, resolvePieceAssetKey: () => '' }
  throw new Error(id)
}, exports)
const Board = exports.default as vue.Component

function asSquare(label: string) {
  return { file: label.charCodeAt(0) - 97, rank: Number(label.slice(1)) - 1 }
}

function squareSections(html: string) {
  return [...html.matchAll(/data-file="(\d+)" data-rank="(\d+)"[\s\S]*?(?=data-file="\d+" data-rank="\d+"|class="board-arrow-overlay"|class="presentation-layer")/g)]
}

const zoneData = [
  [8, [
    ['b8', 'a8 a7 b7 c7 c8'], ['g8', 'f8 f7 g7 h7 h8'],
    ['a5 a4', 'a6 b6 b5 b4 b3 a3'],
    ['d5 e5 d4 e4', 'd6 e6 f6 c6 c5 c4 c3 d3 e3 f3 f4 f5'],
    ['h5 h4', 'h6 g6 g5 g4 g3 h3'],
    ['b1', 'a1 a2 b2 c2 c1'], ['g1', 'f1 f2 g2 h2 h1'],
  ]],
  [12, [
    ['f12 g12', 'e12 e11 f11 g11 h11 h12'],
    ['c11', 'b12 c12 d12 b11 d11 b10 c10 d10'],
    ['j11', 'i12 j12 k12 i11 k11 i10 j10 k10'],
    ['b7 c7 b6 c6', 'a8 b8 c8 d8 a7 d7 a6 d6 a5 b5 c5 d5'],
    ['f7 g7 f6 g6', 'e8 f8 g8 h8 e7 h7 e6 h6 e5 f5 g5 h5'],
    ['j7 k7 j6 k6', 'i8 j8 k8 l8 i7 l7 i6 l6 i5 j5 k5 l5'],
    ['c2', 'b3 c3 d3 b2 d2 b1 c1 d1'],
    ['j2', 'i3 j3 k3 i2 k2 i1 j1 k1'],
    ['f1 g1', 'e1 e2 f2 g2 h2 h1'],
  ]],
] as const

for (const [size, data] of zoneData) {
  const zones = data.map(([circles, arrows], index) => ({
    squares: circles.split(' ').map(asSquare),
    effect_squares: arrows.split(' ').map(asSquare),
    owner: index === 0 ? 'white' : index === data.length - 1 ? 'black' : null,
  }))
  test(`${size}x${size} board marks circles, colors arrow cells, and has no stack`, async () => {
    const html = await renderToString(vue.h(Board, {
      board: { size, squares: {}, air_squares: {}, terrain: {} }, pieces: {}, definitions: {},
      occupationZones: zones, selectedPieceId: null, movableSquares: [], attackSquares: [], dropSquares: [],
    }))
    const sections = squareSections(html)
    const markerLabels = sections.filter(section => section[0].includes('occupation-ring'))
      .map(section => `${String.fromCharCode(97 + Number(section[1]))}${Number(section[2]) + 1}`)
    assert.deepEqual(new Set(markerLabels), new Set(data.flatMap(([circles]) => circles.split(' '))))
    assert.equal(markerLabels.length, data.flatMap(([circles]) => circles.split(' ')).length)
    const first = new Set(data[0][1].split(' ').map(label => { const s = asSquare(label); return `${s.file}_${s.rank}` }))
    const last = new Set(data[data.length - 1][1].split(' ').map(label => { const s = asSquare(label); return `${s.file}_${s.rank}` }))
    for (const section of sections) {
      const id = `${section[1]}_${section[2]}`
      assert.equal(section[0].includes('class="occupation-effect occupation-white"'), first.has(id), `white effect at ${id}`)
      assert.equal(section[0].includes('class="occupation-effect occupation-black"'), last.has(id), `black effect at ${id}`)
    }
    assert.doesNotMatch(html, /occupation-stack|스택/)
  })
}
