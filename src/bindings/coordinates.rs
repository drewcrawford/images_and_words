/**
IW standard coordinate system.  This is a cross-platform coordinate in which

```text
           x
      0 ────────▶
      │ ┌───────┐
    y │ │       │
      │ │       │
      │ │       │
      ▼ └───────┘
 ```
*/
#[derive(Debug)]
pub struct RasterCoord2D {
    pub x: u16,
    pub y: u16
}