# Drawing text: `TextParams`

`BmFont::draw_text_ex(text, x, baseline_y, &params)` draws a string; `TextParams` says how. Every
field has a default (`TextParams::default()`), so set only what you need:

```rust
let params = TextParams {
    scale: 8.0,
    ink: Ink::Set(1),
    effect: Effect::Shadow,
    edge: Color::from_rgba(69, 36, 4, 255),
    ..Default::default()
};
font.draw_text_ex("Next Turn", x, baseline, &params);
```

Distances in `effect`, `shadow`, `outline` and `highlight` are in **font pixels**: they are
multiplied by `scale`, so a pixel font keeps its look at any size.

## Drawing order

Back to front: `effect` copies, then `shadow`, then `outline`, then the letters (`ink`), then the
`highlight` on top.

## Fields

| Field | Default | What it does |
|---|---|---|
| `scale` | `1.0` | Screen pixels per font pixel. `0` or less draws nothing. |
| `ink` | `Ink::Tint(WHITE)` | The letters' colour, see below. |
| `align` | `Align::Left` | `Left`, `Center` or `Right`: where each line sits against the `x` given to `draw_text_ex`. |
| `effect` | `Effect::Plain` | Classic copies behind the text, in the `edge` colour, see below. |
| `edge` | `BLACK` | The colour of the `effect` copies. With alpha below 1 the copies are painted as one shape, each pixel once, so overlaps are not blended twice and the colour takes what is under it. |
| `shadow` | `None` | One more copy, `Shadow { offset, color }`, moved by `offset` font pixels. |
| `outline` | `None` | Copies on the sides given by `Outline { color, shape }` (`Full`, `Cross`, `CrossAndLowerRight`). |
| `highlight` | `None` | A light-source reflection: the letters' top-left corner pixels (nothing above or to the left) in this colour, over the ink. |
| `highlight_edge` | `None` | The rest of the top and left edges (pixels bare on only one side). `None` uses the colour of `highlight`. Give it a more transparent colour for a softer edge. |
| `snap` | `true` | Rounds glyph positions to whole screen pixels so pixel fonts do not shimmer. |

## `Ink`: the letters' colour

- `Ink::Tint(color)`: the lightest colour set multiplied by `color` (alpha included). For a font
  with one set, simply a tint.
- `Ink::Set(n)`: colour set `n` by number, in its own colours.
- `Ink::Nearest(color)`: the set whose average colour is closest to `color`.
- `Ink::Gradient { top, bottom }`: the lightest set tinted by a vertical gradient over each line.

An atlas may stack colour sets (copies of the glyph grid in other colours, one `scaleH` apart).

## `Effect`: copies behind the text

Offsets in font pixels, drawn in `edge`:

- `Plain`: none.
- `Shadow`: three copies to the lower right, (1,1), (0,1), (1,0).
- `ShadowUp`: three copies to the upper left, (-1,0), (-1,-1), (0,-1).
- `ShadowHeavy`: `Shadow` and a second ring two pixels out.
- `Outline`: copies on all eight sides.
- `OutlineHeavy`: `Outline` and five more copies to the right, below and the lower right.

Only one `effect` per draw. For two shadows of different colours (for example a light one up and
left and a dark one down and right), draw the text twice: first with `Shadow` and the dark
`edge`, then over it with `ShadowUp` and the light `edge`. The ink of the second pass covers the
first.

## The reflection highlight

`highlight` and `highlight_edge` paint the top and left edges of the letters in a lighter colour,
as if lit from the upper left. A letter pixel is a corner when the pixel above it and the pixel to
its left are both empty; it is an edge pixel when only one of them is. Pixels with solid neighbours
on both sides get nothing. Use translucent colours to let the ink show through.

```rust
highlight: Some(Color::from_rgba(255, 236, 130, 200)),      // corners
highlight_edge: Some(Color::from_rgba(255, 236, 130, 90)),  // the rest of the edges
```

The highlight reads the atlas back from the texture once (`get_texture_data`), as the colour sets
and translucent effects do.
