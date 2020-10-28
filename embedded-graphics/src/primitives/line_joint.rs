//! Thick line joint.

use crate::{
    geometry::Point,
    primitives::{
        line::{Intersection, Side},
        Line, Triangle,
    },
    style::StrokeAlignment,
};

/// Joint kind
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum JointKind {
    /// Mitered (sharp point)
    Miter,

    /// Bevelled (flattened point)
    Bevel {
        /// The triangle used to draw the bevel itself.
        filler_triangle: Triangle,

        /// Line filling the outside of the bevel.
        filler_line: Line,

        /// Left side or right side?
        side: Side,
    },

    /// Degenerate (angle between lines is too small to properly render stroke).
    ///
    /// Degenerate corners are rendered with a bevel.
    Degenerate {
        /// The triangle used to fill in the corner.
        filler_triangle: Triangle,

        /// Line filling the outside of the bevel.
        filler_line: Line,

        /// Left side or right side?
        side: Side,
    },

    /// Essentially no joint (both lines are colinear)
    Colinear,

    /// The starting "joint" of a line
    Start,

    /// The ending "joint" of a line
    End,
}

// DELETEME: Remove when debugging in demo is no longer required.
use std::fmt;
impl fmt::Display for JointKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Miter => f.write_str("Miter"),
            Self::Bevel { .. } => f.write_str("Bevel"),
            Self::Degenerate { .. } => f.write_str("Degenerate"),
            Self::Colinear => f.write_str("Colinear"),
            Self::Start => f.write_str("Start"),
            Self::End => f.write_str("End"),
        }
    }
}

/// The left/right corners that make up the start or end edge of a thick line.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct EdgeCorners {
    /// Left side point.
    pub left: Point,

    /// Right side point.
    pub right: Point,
}

/// A joint between two lines.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct LineJoint {
    /// Joint kind.
    pub kind: JointKind,

    /// Corners comprising the ending edge of the line that ends at this joint.
    pub first_edge_end: EdgeCorners,

    /// Corners comprising the start edge of the line that begins at this joint.
    pub second_edge_start: EdgeCorners,
}

impl LineJoint {
    /// Create a starting joint.
    ///
    /// `first_edge_end` and `second_edge_start` are set to the same points.
    pub fn start(start: Point, mid: Point, width: u32, alignment: StrokeAlignment) -> Self {
        let line = Line::new(start, mid);

        let (l, r) = line.extents(width, alignment);

        let points = EdgeCorners {
            left: l.start,
            right: r.start,
        };

        Self {
            kind: JointKind::Start,
            first_edge_end: points,
            second_edge_start: points,
        }
    }

    /// Create an ending joint.
    ///
    /// `first_edge_end` and `second_edge_start` are set to the same points.
    pub fn end(mid: Point, end: Point, width: u32, alignment: StrokeAlignment) -> Self {
        let line = Line::new(mid, end);

        let (l, r) = line.extents(width, alignment);

        let points = EdgeCorners {
            left: l.end,
            right: r.end,
        };

        Self {
            kind: JointKind::End,
            first_edge_end: points,
            second_edge_start: points,
        }
    }

    /// Empty joint
    pub fn empty() -> Self {
        Self {
            kind: JointKind::End,
            first_edge_end: EdgeCorners {
                left: Point::zero(),
                right: Point::zero(),
            },
            second_edge_start: EdgeCorners {
                left: Point::zero(),
                right: Point::zero(),
            },
        }
    }

    /// Compute a joint.
    pub fn from_points(
        start: Point,
        mid: Point,
        end: Point,
        width: u32,
        alignment: StrokeAlignment,
    ) -> Self {
        let first_line = Line::new(start, mid);
        let second_line = Line::new(mid, end);

        // Miter length limit is double the line width (but squared to avoid sqrt() costs)
        let miter_limit = (width * 2).pow(2);

        // Left and right edges of thick first segment
        let (first_edge_left, first_edge_right) = first_line.extents(width, alignment);
        // Left and right edges of thick second segment
        let (second_edge_left, second_edge_right) = second_line.extents(width, alignment);

        if let (
            Intersection::Point {
                point: l_intersection,
                outer_side,
                ..
            },
            Intersection::Point {
                point: r_intersection,
                ..
            },
        ) = (
            second_edge_left.line_intersection(&first_edge_left),
            second_edge_right.line_intersection(&first_edge_right),
        ) {
            let first_segment_start_edge = Line::new(first_edge_left.start, first_edge_right.start);
            let second_segment_end_edge = Line::new(second_edge_left.end, second_edge_right.end);

            let self_intersection_l = first_segment_start_edge
                .segment_intersection(&second_edge_left)
                || second_segment_end_edge.segment_intersection(&first_edge_left);

            let self_intersection_r = first_segment_start_edge
                .segment_intersection(&second_edge_right)
                || second_segment_end_edge.segment_intersection(&first_edge_right);

            // Normal line: non-overlapping line end caps
            if !self_intersection_r && !self_intersection_l {
                // Distance from midpoint to miter outside end point.
                let miter_length_squared = Line::new(
                    mid,
                    match outer_side {
                        Side::Left => l_intersection,
                        Side::Right => r_intersection,
                    },
                )
                .length_squared();

                // Intersection is within limit at which it will be chopped off into a bevel, so return
                // a miter.
                if miter_length_squared <= miter_limit {
                    let corners = EdgeCorners {
                        left: l_intersection,
                        right: r_intersection,
                    };

                    Self {
                        kind: JointKind::Miter,
                        first_edge_end: corners,
                        second_edge_start: corners,
                    }
                }
                // Miter is too long, chop it into bevel-style corner
                else {
                    match outer_side {
                        Side::Right => Self {
                            kind: JointKind::Bevel {
                                /// Must be counter-clockwise
                                filler_triangle: Triangle::new(
                                    first_edge_right.end,
                                    l_intersection,
                                    second_edge_right.start,
                                ),
                                filler_line: Line::new(
                                    first_edge_right.end,
                                    second_edge_right.start,
                                ),
                                side: outer_side,
                            },
                            first_edge_end: EdgeCorners {
                                left: l_intersection,
                                right: first_edge_right.end,
                            },
                            second_edge_start: EdgeCorners {
                                left: l_intersection,
                                right: second_edge_right.start,
                            },
                        },
                        Side::Left => Self {
                            kind: JointKind::Bevel {
                                /// Must be clockwise
                                filler_triangle: Triangle::new(
                                    first_edge_left.end,
                                    second_edge_left.start,
                                    r_intersection,
                                ),
                                filler_line: Line::new(first_edge_left.end, second_edge_left.start),
                                side: outer_side,
                            },
                            first_edge_end: EdgeCorners {
                                left: first_edge_left.end,
                                right: r_intersection,
                            },
                            second_edge_start: EdgeCorners {
                                left: second_edge_left.start,
                                right: r_intersection,
                            },
                        },
                    }
                }
            }
            // Line segments overlap (degenerate)
            else {
                Self {
                    kind: match outer_side {
                        Side::Left => JointKind::Degenerate {
                            filler_triangle: Triangle::new(
                                first_edge_left.end,
                                first_edge_right.end,
                                second_edge_left.start,
                            ),
                            filler_line: Line::new(first_edge_left.end, second_edge_left.start),
                            side: outer_side,
                        },
                        Side::Right => JointKind::Degenerate {
                            filler_triangle: Triangle::new(
                                first_edge_left.end,
                                first_edge_right.end,
                                second_edge_right.start,
                            ),
                            filler_line: Line::new(first_edge_right.end, second_edge_right.start),
                            side: outer_side,
                        },
                    },
                    first_edge_end: EdgeCorners {
                        left: first_edge_left.end,
                        right: first_edge_right.end,
                    },
                    second_edge_start: EdgeCorners {
                        left: second_edge_left.start,
                        right: second_edge_right.start,
                    },
                }
            }
        }
        // Lines are colinear
        else {
            Self {
                kind: JointKind::Colinear,
                first_edge_end: EdgeCorners {
                    left: first_edge_left.end,
                    right: first_edge_right.end,
                },
                second_edge_start: EdgeCorners {
                    left: second_edge_left.start,
                    right: second_edge_right.start,
                },
            }
        }
    }

    fn cap(&self, cap: &EdgeCorners) -> [Option<Line>; 2] {
        let midpoint = match self.kind {
            JointKind::Bevel { filler_line, .. } | JointKind::Degenerate { filler_line, .. } => {
                filler_line.midpoint()
            }
            _ => return [Line::new(cap.left, cap.right).into(), None],
        };

        let l1 = Line::new(cap.left, midpoint);
        let l2 = Line::new(midpoint, cap.right);

        [l1.into(), l2.into()]
    }

    /// Start node bevel line(s).
    ///
    /// If the joint is a bevel joint, this will return two lines, otherwise one.
    pub fn start_cap_lines(&self) -> [Option<Line>; 2] {
        self.cap(&self.second_edge_start)
    }

    /// End node bevel line(s).
    ///
    /// If the joint is a bevel joint, this will return two lines, otherwise one.
    pub fn end_cap_lines(&self) -> [Option<Line>; 2] {
        self.cap(&self.first_edge_end)
    }

    /// Check whether the joint is degenerate (where the end edge of one line intersects the edge
    /// of the other).
    pub fn is_degenerate(&self) -> bool {
        match self.kind {
            JointKind::Degenerate { .. } => true,
            _ => false,
        }
    }

    /// Check whether the two base lines are colinear.
    ///
    /// This also checks for parallelism, but since both lines share a vertex, they can only ever
    /// be colinear.
    pub fn is_colinear(&self) -> bool {
        match self.kind {
            JointKind::Colinear => true,
            _ => false,
        }
    }

    /// Get the filler triangle (if any).
    pub fn filler(&self) -> Option<Triangle> {
        match self.kind {
            JointKind::Bevel {
                filler_triangle, ..
            }
            | JointKind::Degenerate {
                filler_triangle, ..
            } => Some(filler_triangle),
            _ => None,
        }
    }

    /// Returns whether the joint is the last one.
    pub fn is_end_joint(&self) -> bool {
        match self.kind {
            JointKind::End => true,
            _ => false,
        }
    }
}
