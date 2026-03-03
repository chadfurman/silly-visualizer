# Planning

Design documents and implementation plans for upcoming features.

## Documents

| Document | Status | Description |
|----------|--------|-------------|
| [Music Visualizer Design](2026-03-03-music-visualizer-design.md) | Implemented | Original design for the core visualizer |
| [Music Visualizer Implementation](2026-03-03-music-visualizer-implementation.md) | Complete | Step-by-step implementation plan for v0.1 |
| [Evolutionary Scenes Design](2026-03-03-evolutionary-scenes-design.md) | Approved | Genome-driven scenes with multi-generational lineage |
| [Evolutionary Scenes Implementation](2026-03-03-evolutionary-scenes-implementation.md) | Pending | 10-task TDD plan for the genome system |

## Current Priority

The evolutionary scenes system is next. It addresses:
- Constant camera/shape motion even without audio ([details](../architecture/shader-controls.md#problem-constant-motion-without-audio))
- Beat only affecting color, not geometry ([details](../architecture/shader-controls.md#problem-beat-only-affects-color))
- Lack of visual variety between sessions
