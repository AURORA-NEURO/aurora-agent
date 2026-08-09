//! Coordinate frames, anatomical orientation, and what a position means.
//!
//! Blueprint 28.13 names the failure exactly: "array shapes align while physical coordinates do
//! not". Two volumes with the same `[240, 240, 155]` shape can disagree on which end of the
//! first axis is the patient's left, and nothing in the array says so. 39.05 therefore protects
//! coordinate system, orientation and scale alongside units.
//!
//! The distinction this module enforces is between quantities that survive a change of frame
//! and quantities that do not. A tumour diameter is a length: it is the same 24 mm in RAS, in
//! LPS, and in voxel space with isotropic 1 mm spacing. A tumour *centroid* is a position:
//! `[62, -18, 31]` in RAS and `[-62, 18, 31]` in LPS are the same point, and comparing the two
//! triples numerically reports a 128 mm displacement that does not exist.
//!
//! From that follows the rule the crate is built around. A [`Position`] whose frame is
//! [`FrameBinding::Unstated`] is not comparable with anything, *including another unstated
//! position and including itself*. Silence matching silence is the failure mode: two undeclared
//! frames look identical because neither said anything, and the comparison reads as agreement.
//! `bioprism-bioeval` refuses undeclared frames on the same grounds.
//!
//! Not implemented, deliberately: no affine matrices, no resampling, no registration. This
//! module says whether two positions are in the same frame and refuses when they are not; it
//! does not move points between frames. A frame change is a transport that belongs to whichever
//! component owns the registration, and inventing one here would fabricate the transform.

use crate::error::{FrameError, Incomparability};
use crate::reference::ReferenceSpace;
use crate::unit::{Dimension, Quantity, Unit};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The three anatomical axes, without a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnatomicalAxis {
    LeftRight,
    AnteriorPosterior,
    SuperiorInferior,
}

impl AnatomicalAxis {
    pub fn as_str(self) -> &'static str {
        match self {
            AnatomicalAxis::LeftRight => "left-right",
            AnatomicalAxis::AnteriorPosterior => "anterior-posterior",
            AnatomicalAxis::SuperiorInferior => "superior-inferior",
        }
    }

    fn index(self) -> usize {
        match self {
            AnatomicalAxis::LeftRight => 0,
            AnatomicalAxis::AnteriorPosterior => 1,
            AnatomicalAxis::SuperiorInferior => 2,
        }
    }
}

/// One end of an anatomical axis: the direction an array index increases toward.
///
/// Read the letters as "increasing index moves toward". `RAS` means index 0 grows rightward,
/// index 1 grows anteriorly, index 2 grows superiorly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisDirection {
    Right,
    Left,
    Anterior,
    Posterior,
    Superior,
    Inferior,
}

impl AxisDirection {
    pub fn axis(self) -> AnatomicalAxis {
        match self {
            AxisDirection::Right | AxisDirection::Left => AnatomicalAxis::LeftRight,
            AxisDirection::Anterior | AxisDirection::Posterior => {
                AnatomicalAxis::AnteriorPosterior
            }
            AxisDirection::Superior | AxisDirection::Inferior => {
                AnatomicalAxis::SuperiorInferior
            }
        }
    }

    /// `+1` for the direction NIfTI's RAS reference treats as positive, `-1` for its opposite.
    pub fn sign(self) -> i8 {
        match self {
            AxisDirection::Right | AxisDirection::Anterior | AxisDirection::Superior => 1,
            AxisDirection::Left | AxisDirection::Posterior | AxisDirection::Inferior => -1,
        }
    }

    pub fn letter(self) -> char {
        match self {
            AxisDirection::Right => 'R',
            AxisDirection::Left => 'L',
            AxisDirection::Anterior => 'A',
            AxisDirection::Posterior => 'P',
            AxisDirection::Superior => 'S',
            AxisDirection::Inferior => 'I',
        }
    }

    pub fn from_letter(letter: char) -> Result<Self, FrameError> {
        match letter.to_ascii_uppercase() {
            'R' => Ok(AxisDirection::Right),
            'L' => Ok(AxisDirection::Left),
            'A' => Ok(AxisDirection::Anterior),
            'P' => Ok(AxisDirection::Posterior),
            'S' => Ok(AxisDirection::Superior),
            'I' => Ok(AxisDirection::Inferior),
            other => Err(FrameError::UnknownAxisLetter {
                letter: other.to_string(),
            }),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            AxisDirection::Right => AxisDirection::Left,
            AxisDirection::Left => AxisDirection::Right,
            AxisDirection::Anterior => AxisDirection::Posterior,
            AxisDirection::Posterior => AxisDirection::Anterior,
            AxisDirection::Superior => AxisDirection::Inferior,
            AxisDirection::Inferior => AxisDirection::Superior,
        }
    }
}

/// Chirality of an axis triple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Handedness {
    Right,
    Left,
}

impl Handedness {
    pub fn as_str(self) -> &'static str {
        match self {
            Handedness::Right => "right-handed",
            Handedness::Left => "left-handed",
        }
    }
}

/// An axis-direction triple, such as `RAS` (NIfTI) or `LPS` (DICOM).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Orientation {
    axes: [AxisDirection; 3],
}

impl Orientation {
    /// NIfTI's convention.
    pub const RAS: Orientation = Orientation {
        axes: [
            AxisDirection::Right,
            AxisDirection::Anterior,
            AxisDirection::Superior,
        ],
    };

    /// DICOM's convention.
    pub const LPS: Orientation = Orientation {
        axes: [
            AxisDirection::Left,
            AxisDirection::Posterior,
            AxisDirection::Superior,
        ],
    };

    /// Rejects triples that name one anatomical axis twice.
    ///
    /// `RAR` is not an unusual orientation, it is a corrupt header: it leaves the
    /// superior-inferior axis undescribed while describing left-right twice.
    pub fn new(axes: [AxisDirection; 3]) -> Result<Self, FrameError> {
        for (i, first) in axes.iter().enumerate() {
            for second in axes.iter().skip(i + 1) {
                if first.axis() == second.axis() {
                    return Err(FrameError::DegenerateOrientation {
                        code: axes.iter().map(|a| a.letter()).collect(),
                        axis: first.axis().as_str().to_string(),
                    });
                }
            }
        }
        Ok(Orientation { axes })
    }

    /// Parses a three-letter code such as `"RAS"` or `"lps"`.
    pub fn parse(code: &str) -> Result<Self, FrameError> {
        let letters: Vec<char> = code.chars().collect();
        if letters.len() != 3 {
            return Err(FrameError::WrongCodeLength {
                code: code.to_string(),
                length: letters.len(),
            });
        }
        let axes = [
            AxisDirection::from_letter(letters[0])?,
            AxisDirection::from_letter(letters[1])?,
            AxisDirection::from_letter(letters[2])?,
        ];
        Orientation::new(axes)
    }

    pub fn code(&self) -> String {
        self.axes.iter().map(|a| a.letter()).collect()
    }

    pub fn directions(&self) -> [AxisDirection; 3] {
        self.axes
    }

    /// Chirality, as the determinant sign of the signed permutation against RAS.
    ///
    /// RAS and LPS are *both* right-handed, which is why handedness alone can never certify
    /// that two datasets agree: LPS is RAS with two axes negated, and two negations cancel in
    /// the determinant while very much not cancelling in the left-right label.
    pub fn handedness(&self) -> Handedness {
        let mut matrix = [[0i8; 3]; 3];
        for (slot, direction) in self.axes.iter().enumerate() {
            matrix[direction.axis().index()][slot] = direction.sign();
        }
        let det = i32::from(matrix[0][0])
            * (i32::from(matrix[1][1]) * i32::from(matrix[2][2])
                - i32::from(matrix[1][2]) * i32::from(matrix[2][1]))
            - i32::from(matrix[0][1])
                * (i32::from(matrix[1][0]) * i32::from(matrix[2][2])
                    - i32::from(matrix[1][2]) * i32::from(matrix[2][0]))
            + i32::from(matrix[0][2])
                * (i32::from(matrix[1][0]) * i32::from(matrix[2][1])
                    - i32::from(matrix[1][1]) * i32::from(matrix[2][0]));
        if det >= 0 {
            Handedness::Right
        } else {
            Handedness::Left
        }
    }

    /// The anatomical axes on which the two orientations point opposite ways.
    ///
    /// Non-empty means a numeric coordinate comparison would mirror the patient. RAS against
    /// LPS reports left-right and anterior-posterior.
    pub fn disagreeing_axes(&self, other: &Orientation) -> Vec<AnatomicalAxis> {
        let mut out = Vec::new();
        for axis in [
            AnatomicalAxis::LeftRight,
            AnatomicalAxis::AnteriorPosterior,
            AnatomicalAxis::SuperiorInferior,
        ] {
            let mine = self.axes.iter().find(|d| d.axis() == axis);
            let theirs = other.axes.iter().find(|d| d.axis() == axis);
            if let (Some(mine), Some(theirs)) = (mine, theirs) {
                if mine != theirs {
                    out.push(axis);
                }
            }
        }
        out
    }
}

impl fmt::Display for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Whether coordinates are array indices or physical units.
///
/// Voxel indices are meaningless outside the grid that produced them: index 60 on a 1 mm grid
/// and index 60 on a 0.5 mm grid are 30 mm apart. The grid identifier is therefore part of the
/// space, not an annotation on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "space", rename_all = "snake_case")]
pub enum CoordinateSpace {
    Voxel { grid: String },
    World,
}

impl fmt::Display for CoordinateSpace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoordinateSpace::Voxel { grid } => write!(f, "voxel space of grid {grid}"),
            CoordinateSpace::World => write!(f, "world space"),
        }
    }
}

/// A fully declared coordinate frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frame {
    pub id: String,
    pub space: CoordinateSpace,
    pub orientation: Orientation,
    pub reference: ReferenceSpace,
}

impl Frame {
    pub fn new(
        id: impl Into<String>,
        space: CoordinateSpace,
        orientation: Orientation,
        reference: ReferenceSpace,
    ) -> Self {
        Frame {
            id: id.into(),
            space,
            orientation,
            reference,
        }
    }

    /// A world-space frame in a named template, the common case for atlas coordinates.
    pub fn world(id: impl Into<String>, orientation: Orientation, reference: ReferenceSpace) -> Self {
        Frame::new(id, CoordinateSpace::World, orientation, reference)
    }

    /// Whether two frames describe the same coordinate system.
    ///
    /// Frame identity is not name equality: two frames named differently but agreeing on space,
    /// orientation and reference describe the same system, and two frames sharing a name while
    /// disagreeing on orientation do not.
    pub fn agrees_with(&self, other: &Frame) -> Result<(), Incomparability> {
        if self.space != other.space {
            return Err(Incomparability::SpaceMismatch {
                left: self.space.clone(),
                right: other.space.clone(),
            });
        }
        if self.orientation != other.orientation {
            return Err(Incomparability::OrientationMismatch {
                left: self.orientation.code(),
                right: other.orientation.code(),
            });
        }
        if self.reference != other.reference {
            return Err(Incomparability::FrameMismatch {
                left: self.reference.label(),
                right: other.reference.label(),
            });
        }
        Ok(())
    }
}

/// A frame, or the admission that none was recorded.
///
/// There is no `Default`. Defaulting to RAS would be the exact silent coercion 28.00 forbids:
/// it would turn "nobody wrote down the frame" into "the frame is RAS".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FrameBinding {
    Declared(Frame),
    Unstated,
}

impl FrameBinding {
    pub fn declared(&self) -> Option<&Frame> {
        match self {
            FrameBinding::Declared(frame) => Some(frame),
            FrameBinding::Unstated => None,
        }
    }

    pub fn is_stated(&self) -> bool {
        matches!(self, FrameBinding::Declared(_))
    }
}

/// A point, which only means something inside a frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub coordinates: [f64; 3],
    pub unit: Unit,
    pub frame: FrameBinding,
}

impl Position {
    pub fn new(coordinates: [f64; 3], unit: Unit, frame: FrameBinding) -> Self {
        Position {
            coordinates,
            unit,
            frame,
        }
    }

    /// A position with no recorded frame.
    ///
    /// Constructing one is allowed because ingested data really does arrive like this; what is
    /// not allowed is comparing it.
    pub fn unstated(coordinates: [f64; 3], unit: Unit) -> Self {
        Position {
            coordinates,
            unit,
            frame: FrameBinding::Unstated,
        }
    }

    /// Whether the two positions live in the same frame and the same unit.
    ///
    /// Returns the *first* blocking reason. Unstated frames are checked before anything else,
    /// because when the frame is unknown no other agreement is evidence of anything.
    pub fn comparable_with(&self, other: &Position) -> Result<(), Incomparability> {
        let (Some(mine), Some(theirs)) = (self.frame.declared(), other.frame.declared()) else {
            let side = match (self.frame.is_stated(), other.frame.is_stated()) {
                (false, false) => "both positions",
                (false, true) => "the left position",
                _ => "the right position",
            };
            return Err(Incomparability::UnstatedFrame {
                side: side.to_string(),
            });
        };
        mine.agrees_with(theirs)?;
        if self.unit.symbol != other.unit.symbol {
            return Err(Incomparability::ConversionRequired {
                left_unit: self.unit.symbol.clone(),
                right_unit: other.unit.symbol.clone(),
            });
        }
        Ok(())
    }
}

/// A frame-independent distance.
///
/// The counterpart to [`Position`]: a separation between two points in one frame has the same
/// magnitude in every frame related by a rigid transform, so it carries no frame binding at all.
/// Modelling it as a distinct type is what stops a length being handed to code that expects a
/// located point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub magnitude: Quantity,
}

impl Extent {
    /// Wraps a length-dimensioned quantity.
    ///
    /// Areas and volumes are also frame-independent, so the check is on the dimension being
    /// spatial rather than on it being exactly a length.
    pub fn new(magnitude: Quantity) -> Result<Self, Incomparability> {
        let dimension = magnitude.dimension();
        let spatial = dimension == Dimension::LENGTH
            || dimension == Dimension::AREA
            || dimension == Dimension::VOLUME;
        if spatial {
            Ok(Extent { magnitude })
        } else {
            Err(Incomparability::DimensionMismatch {
                left: "extent".to_string(),
                right: magnitude.unit.symbol.clone(),
                left_dimension: Dimension::LENGTH,
                right_dimension: dimension,
            })
        }
    }

    /// Extents compare across frames; only their units have to agree.
    pub fn comparable_with(&self, other: &Extent) -> Result<(), Incomparability> {
        if self.magnitude.dimension() != other.magnitude.dimension() {
            return Err(Incomparability::DimensionMismatch {
                left: self.magnitude.unit.symbol.clone(),
                right: other.magnitude.unit.symbol.clone(),
                left_dimension: self.magnitude.dimension(),
                right_dimension: other.magnitude.dimension(),
            });
        }
        if self.magnitude.unit.symbol != other.magnitude.unit.symbol {
            return Err(Incomparability::ConversionRequired {
                left_unit: self.magnitude.unit.symbol.clone(),
                right_unit: other.magnitude.unit.symbol.clone(),
            });
        }
        Ok(())
    }
}
