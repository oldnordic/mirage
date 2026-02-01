//! AST-based CFG construction using tree-sitter
//!
//! This module provides a fallback CFG construction method for non-Rust code
//! or when Charon is unavailable. It uses the leader-based algorithm to identify
//! basic block boundaries:
//! - First instruction in function body
//! - Branch targets (consequent/alternative of conditionals)
//! - Instructions after branches (merge points)

use crate::cfg::{BasicBlock, BlockKind, Cfg, EdgeType, Terminator};
use std::collections::{HashMap, HashSet};
use tree_sitter::Node;

/// Build CFG from a function node in tree-sitter AST
///
/// # Arguments
/// * `fn_node` - The function definition node from tree-sitter
/// * `source` - The source code text (for extracting statement text)
///
/// # Returns
/// A control flow graph with basic blocks and edges
pub fn ast_to_cfg(fn_node: Node, source: &str) -> Cfg {
    let builder = CFGBuilder::new(source);
    builder.build_from_function(fn_node)
}

/// CFG builder from tree-sitter AST
///
/// Uses the leader-based algorithm:
/// 1. Identify all leader nodes (block boundaries)
/// 2. Build maximal straight-line sequences (basic blocks)
/// 3. Connect blocks with control flow edges
pub struct CFGBuilder<'a> {
    /// Source code for extracting statement text
    source: &'a str,
    /// The resulting CFG graph
    graph: Cfg,
    /// Leader node IDs (basic block boundaries)
    leaders: HashSet<usize>,
    /// Statements grouped by block ID
    blocks: HashMap<usize, Vec<Node<'a>>>,
    /// Maps tree-sitter node IDs to block IDs
    node_to_block: HashMap<usize, usize>,
    /// Maps block IDs to graph node indices
    node_map: HashMap<usize, petgraph::graph::NodeIndex>,
    /// Next block ID to assign
    next_block_id: usize,
}

impl<'a> CFGBuilder<'a> {
    /// Create a new CFG builder
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            graph: Cfg::new(),
            leaders: HashSet::new(),
            blocks: HashMap::new(),
            node_to_block: HashMap::new(),
            node_map: HashMap::new(),
            next_block_id: 0,
        }
    }

    /// Build a CFG from a function definition node
    pub fn build_from_function(mut self, fn_node: Node<'a>) -> Cfg {
        // Find all leaders in function body
        self.find_leaders(fn_node);

        // Build basic blocks between leaders
        self.build_blocks(fn_node);

        // Connect blocks with edges
        self.connect_edges(fn_node);

        self.graph
    }

    /// Identify leader nodes (basic block boundaries)
    ///
    /// Leaders are:
    /// 1. First statement in function body (entry)
    /// 2. First statement after conditional branches (merge points)
    /// 3. Branch targets (consequent/alternative bodies)
    fn find_leaders(&mut self, fn_node: Node<'a>) {
        let body = self.get_function_body(fn_node);

        // First statement is always a leader (ENTRY)
        if let Some(first) = self.first_statement(body) {
            self.leaders.insert(first.id());
        }

        // Find branch targets and statements after branches
        self.scan_for_leaders(body);
    }

    /// Scan for additional leaders in control flow constructs
    fn scan_for_leaders(&mut self, node: Node<'a>) {
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            match child.kind() {
                "if_statement" | "elif" | "else" => {
                    // Consequent (then branch) first statement is a leader
                    if let Some(consequence) = self.get_consequence(child) {
                        if let Some(first) = self.first_statement(consequence) {
                            self.leaders.insert(first.id());
                        }
                    }
                    // Alternate (else branch) first statement is a leader
                    if let Some(alternate) = self.get_alternate(child) {
                        if let Some(first) = self.first_statement(alternate) {
                            self.leaders.insert(first.id());
                        }
                    }
                    // Statement after control flow is a leader (merge point)
                    if let Some(next) = self.next_sibling(child) {
                        self.leaders.insert(next.id());
                    }
                }
                "while_statement" | "for_statement" | "loop_statement" => {
                    // Loop body first statement is a leader
                    if let Some(body) = self.get_loop_body(child) {
                        if let Some(first) = self.first_statement(body) {
                            self.leaders.insert(first.id());
                        }
                    }
                    // Statement after loop is a leader (exit point)
                    if let Some(next) = self.next_sibling(child) {
                        self.leaders.insert(next.id());
                    }
                }
                "return_statement" | "break_statement" | "continue_statement" => {
                    // Statement after terminator is a leader (if reachable)
                    if let Some(next) = self.next_sibling(child) {
                        self.leaders.insert(next.id());
                    }
                }
                _ => {
                    // Recurse into nested structures
                    self.scan_for_leaders(child);
                }
            }
        }
    }

    /// Build basic blocks from statements between leaders
    fn build_blocks(&mut self, fn_node: Node<'a>) {
        let body = self.get_function_body(fn_node);
        let mut current_block: Vec<Node<'a>> = Vec::new();
        let mut block_id = 0;

        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            // Filter out non-statement nodes (comments, whitespace, etc.)
            if self.is_statement(child) {
                if self.is_leader(child) {
                    // Save previous block if non-empty
                    if !current_block.is_empty() {
                        self.blocks.insert(block_id, current_block);
                        block_id += 1;
                        current_block = Vec::new();
                    }
                }
                current_block.push(child);
                // Track which block this node belongs to
                self.node_to_block.insert(child.id(), block_id);
            }
        }

        // Don't forget the last block
        if !current_block.is_empty() {
            self.blocks.insert(block_id, current_block);
        }
    }

    /// Connect blocks with control flow edges
    fn connect_edges(&mut self, fn_node: Node<'a>) {
        let body = self.get_function_body(fn_node);

        // Create graph nodes for each block
        for (&id, statements) in &self.blocks {
            let kind = if id == 0 {
                BlockKind::Entry
            } else {
                self.classify_block(statements)
            };

            let basic_block = BasicBlock {
                id,
                kind,
                statements: statements
                    .iter()
                    .map(|n| self.node_text(*n))
                    .collect(),
                terminator: self.extract_terminator(statements),
            };

            let node_idx = self.graph.add_node(basic_block);
            self.node_map.insert(id, node_idx);
        }

        // Add edges by analyzing control flow
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if self.is_statement(child) {
                self.add_edges_for_node(child);
            }
        }
    }

    // Helper methods

    /// Get the function body block from a function definition
    fn get_function_body(&self, fn_node: Node<'a>) -> Node<'a> {
        let mut cursor = fn_node.walk();
        let result = fn_node
            .children(&mut cursor)
            .find(|n| n.kind() == "block");
        match result {
            Some(block) => block,
            None => fn_node,
        }
    }

    /// Get the first statement from a block
    fn first_statement(&self, block: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = block.walk();
        for child in block.children(&mut cursor) {
            if self.is_statement(child) {
                return Some(child);
            }
        }
        None
    }

    /// Check if a node is a leader (block boundary)
    fn is_leader(&self, node: Node<'a>) -> bool {
        self.leaders.contains(&node.id())
    }

    /// Check if a node represents a statement (not whitespace/comment)
    fn is_statement(&self, node: Node<'a>) -> bool {
        // Filter out non-statement nodes
        !matches!(
            node.kind(),
            "" | "comment" | "line_comment" | "block_comment" | ";"
        )
    }

    /// Classify a block based on its terminator
    fn classify_block(&self, statements: &[Node<'a>]) -> BlockKind {
        if let Some(last) = statements.last() {
            match last.kind() {
                "return_statement" | "break_statement" | "continue_statement" => {
                    BlockKind::Exit
                }
                _ => BlockKind::Normal,
            }
        } else {
            BlockKind::Normal
        }
    }

    /// Extract the terminator instruction from a block
    fn extract_terminator(&self, statements: &[Node<'a>]) -> Terminator {
        if let Some(last) = statements.last() {
            match last.kind() {
                "return_statement" => Terminator::Return,
                "break_statement" | "continue_statement" => Terminator::Return,
                "if_statement" => Terminator::SwitchInt {
                    targets: vec![],
                    otherwise: 0,
                },
                "while_statement" | "for_statement" | "loop_statement" => {
                    Terminator::SwitchInt {
                        targets: vec![],
                        otherwise: 0,
                    }
                }
                _ => Terminator::Goto { target: 0 },
            }
        } else {
            Terminator::Return
        }
    }

    /// Get the text content of a node
    fn node_text(&self, node: Node<'a>) -> String {
        self.source[node.byte_range()].to_string()
    }

    /// Get the consequence (then branch) of a conditional
    fn get_consequence(&self, node: Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("consequence")
            .or_else(|| node.child_by_field_name("then"))
            .or_else(|| {
                // Some languages use "body" for the then branch
                node.child_by_field_name("body")
            })
    }

    /// Get the alternative (else branch) of a conditional
    fn get_alternate(&self, node: Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("alternative")
            .or_else(|| node.child_by_field_name("else"))
    }

    /// Get the body of a loop construct
    fn get_loop_body(&self, node: Node<'a>) -> Option<Node<'a>> {
        node.child_by_field_name("body")
    }

    /// Get the next sibling node
    fn next_sibling(&self, node: Node<'a>) -> Option<Node<'a>> {
        node.next_sibling()
    }

    /// Add control flow edges for a given node
    fn add_edges_for_node(&mut self, node: Node<'a>) {
        match node.kind() {
            "if_statement" | "elif" => {
                self.handle_if(node);
            }
            "while_statement" | "for_statement" | "loop_statement" => {
                self.handle_loop(node);
            }
            _ => {}
        }
    }

    /// Handle if statement edge creation
    fn handle_if(&mut self, if_node: Node<'a>) {
        // Find blocks for condition, then branch, else branch, merge point
        let condition_block = self.find_block_containing(if_node.id());

        // Collect all blocks first to avoid borrow issues
        let consequence = self.get_consequence(if_node);
        let then_block = consequence.and_then(|c| self.find_block_for_node(Some(c)));
        let alternate = self.get_alternate(if_node);
        let else_block = alternate.and_then(|a| self.find_block_for_node(Some(a)));
        let after_block = self.find_block_for_node(self.next_sibling(if_node));

        let cond_idx = self.node_map.get(&condition_block).copied();
        let then_idx = then_block.and_then(|b| self.node_map.get(&b).copied());
        let else_idx = else_block.and_then(|b| self.node_map.get(&b).copied());
        let after_idx = after_block.and_then(|b| self.node_map.get(&b).copied());

        // Track which blocks need to be marked as exit
        let mut mark_then_exit = false;
        let mut mark_else_exit = false;

        // Add edges: condition -> then, condition -> else
        if let (Some(cond), Some(then_blk)) = (cond_idx, then_idx) {
            self.graph.add_edge(cond, then_blk, EdgeType::TrueBranch);
            // Then branch continues to merge point
            if let Some(after) = after_idx {
                self.graph.add_edge(then_blk, after, EdgeType::Fallthrough);
            } else {
                // No after block, mark as potential exit
                mark_then_exit = true;
            }
        }

        if let (Some(cond), Some(else_blk)) = (cond_idx, else_idx) {
            self.graph.add_edge(cond, else_blk, EdgeType::FalseBranch);
            // Else branch continues to merge point
            if let Some(after) = after_idx {
                self.graph.add_edge(else_blk, after, EdgeType::Fallthrough);
            } else {
                // No after block, mark as potential exit
                mark_else_exit = true;
            }
        }

        // Mark exit blocks after done with borrows
        if mark_then_exit {
            if let Some(bid) = then_block {
                self.mark_block_exit(&bid);
            }
        }
        if mark_else_exit {
            if let Some(bid) = else_block {
                self.mark_block_exit(&bid);
            }
        }
    }

    /// Handle loop statement edge creation
    fn handle_loop(&mut self, loop_node: Node<'a>) {
        let header_block = self.find_block_containing(loop_node.id());
        let body = self.get_loop_body(loop_node);
        let body_block = body.and_then(|b| self.find_block_for_node(Some(b)));
        let after_block = self.find_block_for_node(self.next_sibling(loop_node));

        let header_idx = self.node_map.get(&header_block).copied();
        let body_idx = body_block.and_then(|b| self.node_map.get(&b).copied());
        let after_idx = after_block.and_then(|b| self.node_map.get(&b).copied());

        // Edges: header -> body (loop), body -> header (back), header -> after (exit)
        if let (Some(header), Some(body_blk)) = (header_idx, body_idx) {
            self.graph.add_edge(header, body_blk, EdgeType::TrueBranch);
            self.graph.add_edge(body_blk, header, EdgeType::LoopBack);
        }

        if let (Some(header), Some(after)) = (header_idx, after_idx) {
            self.graph.add_edge(header, after, EdgeType::LoopExit);
        }
    }

    /// Find the block ID containing a given node
    fn find_block_containing(&self, node_id: usize) -> usize {
        for (&id, statements) in &self.blocks {
            if statements.iter().any(|n| n.id() == node_id) {
                return id;
            }
        }
        0 // Default to block 0
    }

    /// Find the block ID for a node's first statement
    fn find_block_for_node(&self, node: Option<Node<'a>>) -> Option<usize> {
        node.and_then(|n| self.first_statement(n))
            .and_then(|first| self.node_to_block.get(&first.id()).copied())
    }

    /// Mark a block as an exit block
    fn mark_block_exit(&mut self, block_id: &usize) {
        if let Some(idx) = self.node_map.get(block_id) {
            if let Some(weight) = self.graph.node_weight_mut(*idx) {
                weight.kind = BlockKind::Exit;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cfg_builder_new() {
        let source = "fn test() { return; }";
        let builder = CFGBuilder::new(source);
        assert_eq!(builder.source, source);
        assert_eq!(builder.next_block_id, 0);
        assert!(builder.leaders.is_empty());
        assert!(builder.blocks.is_empty());
    }

    #[test]
    fn test_leader_detection_empty() {
        let builder = CFGBuilder::new("");
        assert!(builder.leaders.is_empty());
    }

    #[test]
    fn test_block_kind_classification() {
        let builder = CFGBuilder::new("");
        // Empty block is Normal
        assert_eq!(builder.classify_block(&[]), BlockKind::Normal);
    }

    #[test]
    fn test_terminator_extraction_return() {
        let builder = CFGBuilder::new("");
        // Empty slice -> Return terminator (default for empty)
        assert_eq!(builder.extract_terminator(&[]), Terminator::Return);
    }

    #[test]
    fn test_find_block_containing_empty() {
        let builder = CFGBuilder::new("");
        // No blocks exist, should return default 0
        assert_eq!(builder.find_block_containing(999), 0);
    }

    #[test]
    fn test_find_block_for_node_none() {
        let builder = CFGBuilder::new("");
        // None node returns None
        assert_eq!(builder.find_block_for_node(None), None);
    }

    #[test]
    fn test_cfg_builder_state_initialization() {
        let source = "fn example() { let x = 1; }";
        let builder = CFGBuilder::new(source);

        // Verify initial state
        assert_eq!(builder.source, source);
        assert_eq!(builder.next_block_id, 0);
        assert!(builder.leaders.is_empty());
        assert!(builder.blocks.is_empty());
        assert!(builder.node_to_block.is_empty());
        assert!(builder.node_map.is_empty());
        assert_eq!(builder.graph.node_count(), 0);
        assert_eq!(builder.graph.edge_count(), 0);
    }
}
