use std::{
    collections::{
        HashMap,
        hash_map::{Entry, Values},
    },
    error::Error,
    fmt::Display,
};

use dns_lib::{
    query::question::Question,
    resource_record::{rclass::RClass, rtype::RType},
    types::{
        c_domain_name::CDomainName,
        label::{CaseInsensitive, Label, OwnedLabel},
    },
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TreeCacheError {
    NonFullyQualifiedDomainName(CDomainName),
    InconsistentState(String),
}
impl Error for TreeCacheError {}
impl Display for TreeCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFullyQualifiedDomainName(name) => {
                write!(f, "The domain names '{name}' must be fully qualified")
            }
            Self::InconsistentState(message) => write!(f, "Inconsistent State: {message}"),
        }
    }
}

#[derive(Debug)]
pub struct TreeCache<Records> {
    root_nodes: HashMap<RClass, TreeNode<Records>>,
}

type ChildNodes<Records> = HashMap<OwnedLabel<CaseInsensitive>, TreeNode<Records>>;
pub type MappedRecords<Records> = HashMap<RType, Records>;

#[derive(Debug)]
pub struct TreeNode<Records> {
    children: ChildNodes<Records>,
    pub records: MappedRecords<Records>,
}

impl<Records> TreeCache<Records> {
    #[inline]
    pub fn new() -> Self {
        Self {
            root_nodes: HashMap::new(),
        }
    }

    #[inline]
    pub fn get_or_create_node(
        &mut self,
        question: &Question,
    ) -> Result<&mut TreeNode<Records>, TreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !question.qname().is_fully_qualified() {
            return Err(TreeCacheError::NonFullyQualifiedDomainName(
                question.qname().clone(),
            ));
        }

        // If the node does not exist, create it. Then, we can get a shared reference back out of
        // the map.
        let mut current_node = match self.root_nodes.entry(question.qclass()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let new_node = TreeNode {
                    children: ChildNodes::new(),
                    records: MappedRecords::new(),
                };
                entry.insert(new_node)
            }
        };

        // Note: Skipping first label (root label) because it was already checked.
        for label in question.qname().labels().rev().skip(1) {
            // If the node does not exist, create it. Then, we can get a shared reference back out
            // of the map.
            current_node = match current_node.children.entry(label.as_owned()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let child_node = TreeNode {
                        children: HashMap::new(),
                        records: HashMap::new(),
                    };
                    entry.insert(child_node)
                }
            }
        }

        Ok(current_node)
    }

    #[inline]
    pub fn get_node(
        &self,
        question: &Question,
    ) -> Result<Option<&TreeNode<Records>>, TreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !question.qname().is_fully_qualified() {
            return Err(TreeCacheError::NonFullyQualifiedDomainName(
                question.qname().clone(),
            ));
        }

        let mut current_node;
        if let Some(root_node) = self.root_nodes.get(&question.qclass()) {
            current_node = root_node;
        } else {
            return Ok(None);
        }

        // Note: Skipping first label (root label) because it was already checked.
        for label in question.qname().labels().rev().skip(1) {
            if let Some(child_node) = current_node.children.get(label) {
                current_node = child_node;
            } else {
                return Ok(None);
            }
        }

        Ok(Some(current_node))
    }

    #[inline]
    pub fn remove_node(
        &mut self,
        qname: &CDomainName,
        qclass: &RClass,
    ) -> Result<Option<TreeNode<Records>>, TreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !qname.is_fully_qualified() {
            return Err(TreeCacheError::NonFullyQualifiedDomainName(qname.clone()));
        }

        if qname.is_root() {
            return Ok(self.root_nodes.remove(qclass));
        }

        let mut parent_node;
        if let Some(root_node) = self.root_nodes.get_mut(qclass) {
            parent_node = root_node;
        } else {
            return Ok(None);
        }

        let qlabels = qname.labels();
        // Note: Skipping last label (root label) because it was already checked. Skipping first
        // label since that is the one we want to remove and we need its parent.
        for label in qlabels.skip(1).rev().skip(1) {
            if let Some(child_node) = parent_node.children.get_mut(label) {
                parent_node = child_node;
            } else {
                return Ok(None);
            }
        }

        match qname.labels().next() {
            Some(last_label) => Ok(parent_node.children.remove(last_label)),
            None => Err(TreeCacheError::InconsistentState(format!(
                "Could not determine the last label in the qname '{qname}'"
            ))),
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &TreeNode<Records>> {
        TreeRootIterator::new(self)
    }
}

struct TreeRootIterator<'a, Records: 'a> {
    children_iterator: Values<'a, RClass, TreeNode<Records>>,
    current_child: Option<&'a TreeNode<Records>>,
    current_child_iter: Option<TreeChildIterator<'a, Records>>,
}

impl<'a, Records: 'a> TreeRootIterator<'a, Records> {
    #[inline]
    pub fn new(tree: &'a TreeCache<Records>) -> Self {
        Self {
            children_iterator: tree.root_nodes.values(),
            current_child: None,
            current_child_iter: None,
        }
    }
}

impl<'a, Records: 'a> Iterator for TreeRootIterator<'a, Records> {
    type Item = &'a TreeNode<Records>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Step 1: If there is a child iterator and it has sub-nodes, return those sub-nodes.
        if let Some(current_sub_iter) = self.current_child_iter.as_mut() {
            if let Some(next) = current_sub_iter.next() {
                return Some(next);
            }
        }

        // Step 2: If there was a child iterator which is now empty, return the child to which that
        //         iterator belonged. Clear the state of the current iterator so that this case
        //         does not repeat indefinitely.
        if self.current_child_iter.is_some() {
            self.current_child_iter = None;
            return self.current_child;
        }

        // Step 3: Either the current sub-iterator is consumed or one is not defined. Either way,
        //         need to get the next one if one exists.
        match self.children_iterator.next() {
            Some(next_child) => {
                self.current_child_iter = Some(TreeChildIterator::new(next_child));
                self.current_child = Some(next_child);
                self.next()
            }
            None => None,
        }
    }
}

struct TreeChildIterator<'a, Records: 'a> {
    self_node: Option<&'a TreeNode<Records>>,
    children_iterator: Values<'a, OwnedLabel<CaseInsensitive>, TreeNode<Records>>,
    current_child_iter: Option<Box<Self>>,
}

impl<'a, Records: 'a> TreeChildIterator<'a, Records> {
    #[inline]
    pub fn new(tree: &'a TreeNode<Records>) -> Self {
        Self {
            self_node: Some(tree),
            children_iterator: tree.children.values(),
            current_child_iter: None,
        }
    }
}

impl<'a, Records: 'a> Iterator for TreeChildIterator<'a, Records> {
    type Item = &'a TreeNode<Records>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        // Step 1: If there is a child iterator and it has sub-nodes, return those sub-nodes.
        if let Some(current_child_iter) = self.current_child_iter.as_mut() {
            if let Some(next) = current_child_iter.next() {
                return Some(next);
            }
        }

        // Step 2: Either the current sub-iterator is consumed or one is not defined. Either way,
        //         need to get the next one if one exists. If the iterator is consumed, return and
        //         clear the self node (so that the parent is returned after all of its children).
        match (self.children_iterator.next(), self.self_node) {
            (Some(next_child), _) => {
                self.current_child_iter = Some(Box::new(TreeChildIterator::new(next_child)));
                self.next()
            }
            (None, Some(_)) => {
                let self_node = self.self_node;
                self.self_node = None;
                self_node
            }
            (None, None) => None,
        }
    }
}
