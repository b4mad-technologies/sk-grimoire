use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceNodeRef {
    pub node_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceEdgeRef {
    pub edge_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocationDefinitionRef {
    pub location_def_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct H3CellRef {
    pub cell: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileRef {
    pub substrate_id: String,
    pub coordinate_kind_id: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpatialReference {
    PlaceNode(PlaceNodeRef),
    PlaceEdge(PlaceEdgeRef),
    LocationDefinition(LocationDefinitionRef),
    H3Cell(H3CellRef),
    Tile(TileRef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpatialReferenceValidationError {
    EmptyId { field: &'static str },
    InvalidPlaceNodeId { node_id: String },
    InvalidPlaceEdgeId { edge_id: u64 },
    InvalidH3Cell { value: String },
    TacticalGridRejected { value: String },
    UnknownCoordinateKind { coordinate_kind_id: String },
}

pub fn validate_spatial_reference_for_insert(
    reference: &SpatialReference,
) -> Result<(), SpatialReferenceValidationError> {
    match reference {
        SpatialReference::PlaceNode(place_node) => validate_place_node_id(&place_node.node_id),
        SpatialReference::PlaceEdge(place_edge) => {
            if place_edge.edge_id == 0 {
                Err(SpatialReferenceValidationError::InvalidPlaceEdgeId {
                    edge_id: place_edge.edge_id,
                })
            } else {
                Ok(())
            }
        }
        SpatialReference::LocationDefinition(location) => {
            validate_non_empty_id("location_def_id", &location.location_def_id)
        }
        SpatialReference::H3Cell(cell) => validate_h3_cell_value(cell.cell.to_string()),
        SpatialReference::Tile(tile) => {
            validate_non_empty_id("substrate_id", &tile.substrate_id)?;
            match tile.coordinate_kind_id.as_str() {
                "h3_cell" => validate_h3_cell_value(tile.value.clone()),
                "graph_local" => validate_place_node_id(&tile.value),
                "tactical_grid" => Err(SpatialReferenceValidationError::TacticalGridRejected {
                    value: tile.value.clone(),
                }),
                other => Err(SpatialReferenceValidationError::UnknownCoordinateKind {
                    coordinate_kind_id: other.to_string(),
                }),
            }
        }
    }
}

pub fn validate_non_empty_id(
    field: &'static str,
    value: &str,
) -> Result<(), SpatialReferenceValidationError> {
    if value.is_empty() {
        Err(SpatialReferenceValidationError::EmptyId { field })
    } else {
        Ok(())
    }
}

pub fn validate_place_node_id(node_id: &str) -> Result<(), SpatialReferenceValidationError> {
    match node_id.rsplit_once('/') {
        Some((graph, local)) if !graph.is_empty() && !local.is_empty() && !local.contains('/') => {
            Ok(())
        }
        _ => Err(SpatialReferenceValidationError::InvalidPlaceNodeId {
            node_id: node_id.to_string(),
        }),
    }
}

pub fn validate_h3_cell_value(value: String) -> Result<(), SpatialReferenceValidationError> {
    match value.parse::<u64>() {
        Ok(0) | Err(_) => Err(SpatialReferenceValidationError::InvalidH3Cell { value }),
        Ok(_) => Ok(()),
    }
}
