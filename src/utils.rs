use std::collections::{HashMap, HashSet};

use packet_forge::{FileHash, FileMetadata};
use wg_internal::network::{NodeId, SourceRoutingHeader};

use super::FileEntry;

pub fn check_packet_dest(routing_header: &SourceRoutingHeader, node_id: NodeId) -> bool {
    routing_header.hops.last() == Some(&node_id)
}
