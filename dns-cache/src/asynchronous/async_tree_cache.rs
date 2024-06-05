use std::{collections::HashMap, error::Error, fmt::Display, sync::Arc};

use dns_lib::{resource_record::{rclass::RClass, rtype::RType}, types::c_domain_name::{Label, CDomainName}, query::question::Question};
use tokio::sync::RwLock;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AsyncTreeCacheError {
    NonFullyQualifiedDomainName(CDomainName),
    InconsistentState(String),
}
impl Error for AsyncTreeCacheError {}
impl Display for AsyncTreeCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFullyQualifiedDomainName(name) => write!(f, "The domain names '{name}' must be fully qualified"),
            Self::InconsistentState(message) => write!(f, "Inconsistent State: {message}"),
        }
    }
}

#[derive(Debug)]
pub struct AsyncTreeCache<Records> {
    root_nodes: RwLock<HashMap<RClass, Arc<TreeNode<Records>>>>
}

type ChildNodes<Records> = RwLock<HashMap<Label, Arc<TreeNode<Records>>>>;
pub type MappedRecords<Records> = RwLock<HashMap<RType, Records>>;

#[derive(Debug)]
pub struct TreeNode<Records> {
    children: ChildNodes<Records>,
    pub records: MappedRecords<Records>,
}

impl<Records> AsyncTreeCache<Records> {
    #[inline]
    pub fn new() -> Self {
        Self { root_nodes: RwLock::new(HashMap::new()) }
    }

    #[inline]
    pub async fn get_or_create_node(&self, question: &Question) -> Result<Arc<TreeNode<Records>>, AsyncTreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !question.qname().is_fully_qualified() {
            return Err(AsyncTreeCacheError::NonFullyQualifiedDomainName(question.qname().clone()));
        }

        // If the node does not exist, create it. Then, we can get a shared reference back out of
        // the map.
        let mut current_node;
        let qclass = question.qclass();
        let read_root_node = self.root_nodes.read().await;
        match read_root_node.get(&qclass) {
            Some(root_node) => {
                current_node = root_node.clone();
                drop(read_root_node);
            },
            None => {
                drop(read_root_node);
                let mut write_root_node = self.root_nodes.write().await;
                // Need to check again since the read lock was dropped before the write lock was
                // obtained. The state could have changed in that time.
                match write_root_node.get(&qclass) {
                    Some(root_node) => {
                        current_node = root_node.clone();
                        drop(write_root_node);
                    },
                    None => {
                        let new_node = Arc::new(TreeNode {
                            children: RwLock::new(HashMap::new()),
                            records: RwLock::new(HashMap::new()),
                        });
                        write_root_node.insert(qclass, new_node.clone());
                        drop(write_root_node);
                        current_node = new_node;
                    },
                }
            },
        }

        // Note: Skipping first label (root label) because it was already checked.
        for label in question.qname().iter().rev().skip(1) {
            let lowercase_label = label.as_lower();
            // If the node does not exist, create it. Then, we can get a shared reference back out
            // of the map.
            let read_current_node_children = current_node.children.read().await;
            match read_current_node_children.get(&lowercase_label) {
                Some(child_node) => {
                    let child_node = child_node.clone();
                    drop(read_current_node_children);
                    current_node = child_node;
                },
                None => {
                    drop(read_current_node_children);
                    let mut write_current_node_children = current_node.children.write().await;
                    // Need to check again since the read lock was dropped before the write lock was
                    // obtained. The state could have changed in that time.
                    match write_current_node_children.get(&lowercase_label) {
                        Some(child_node) => {
                            let child_node = child_node.clone();
                            drop(write_current_node_children);
                            current_node = child_node;
                        },
                        None => {
                            let child_node = Arc::new(TreeNode {
                                children: RwLock::new(HashMap::new()),
                                records: RwLock::new(HashMap::new()),
                            });
                            write_current_node_children.insert(lowercase_label.clone(), child_node.clone());
                            drop(write_current_node_children);
                            current_node = child_node;
                        },
                    }
                },
            }
        }

        return Ok(current_node)
    }

    #[inline]
    pub async fn get_node(&self, question: &Question) -> Result<Option<Arc<TreeNode<Records>>>, AsyncTreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !question.qname().is_fully_qualified() {
            return Err(AsyncTreeCacheError::NonFullyQualifiedDomainName(question.qname().clone()));
        }

        let mut current_node;
        let read_root_node = self.root_nodes.read().await;
        if let Some(root_node) = read_root_node.get(&question.qclass()) {
            current_node = root_node.clone();
            drop(read_root_node);
        } else {
            drop(read_root_node);
            return Ok(None);
        }
    
        // Note: Skipping first label (root label) because it was already checked.
        for label in question.qname().iter().rev().skip(1) {
            let lowercase_label = label.as_lower();
            let read_current_node_children = current_node.children.read().await;
            if let Some(child_node) = read_current_node_children.get(&lowercase_label) {
                let child_node = child_node.clone();
                drop(read_current_node_children);
                current_node = child_node;
            } else {
                drop(read_current_node_children);
                return Ok(None);
            }
        }

        return Ok(Some(current_node));
    }

    #[inline]
    pub async fn remove_node(&self, qname: &CDomainName, qclass: &RClass) -> Result<Option<Arc<TreeNode<Records>>>, AsyncTreeCacheError> {
        // Checks if domain name ends in root node.
        // The root node of the cache is the root label so if the domain name is not
        // fully qualified, then it is not possible for the domain to be in the cache.
        if !qname.is_fully_qualified() {
            return Err(AsyncTreeCacheError::NonFullyQualifiedDomainName(qname.clone()));
        }

        if qname.is_root() {
            let mut write_root_nodes = self.root_nodes.write().await;
            let result = write_root_nodes.remove(qclass);
            drop(write_root_nodes);
            return Ok(result);
        }

        let mut parent_node;
        let read_root_nodes = self.root_nodes.read().await;
        if let Some(root_node) = read_root_nodes.get(qclass) {
            parent_node = root_node.clone();
            drop(read_root_nodes);
        } else {
            drop(read_root_nodes);
            return Ok(None);
        }

        let qlabels = qname.as_vec();
        // Note: Skipping last label (root label) because it was already checked. Skipping first
        // label since that is the one we want to remove and we need its parent.
        for label in qlabels[1..qlabels.len()-1].iter().rev() {
            let lowercase_label = label.as_lower();
            let read_children = parent_node.children.read().await;
            if let Some(child_node) = read_children.get(&lowercase_label) {
                let next_parent_node = child_node.clone();
                drop(read_children);
                parent_node = next_parent_node;
            } else {
                drop(read_children);
                return Ok(None);
            }
        }

        let mut write_children = parent_node.children.write().await;
        let result = write_children.remove(&qlabels[0]);
        drop(write_children);
        return Ok(result);
    }
}
