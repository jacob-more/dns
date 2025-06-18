use dns_lib::types::c_domain_name::CDomainName;

/// (Original) https://datatracker.ietf.org/doc/html/rfc7816
/// (Updated)  https://datatracker.ietf.org/doc/html/rfc9156
pub enum QNameMinimizer<'a, I>
where
    I: 'a + Iterator<Item = CDomainName> + ExactSizeIterator + DoubleEndedIterator,
{
    None {
        qname: &'a CDomainName,
        repeat_n: usize,
    },
    LimitedMinimizer {
        qname: &'a CDomainName,
        remaining_minimized_qnames: usize,
        qname_iter: I,
    },
}

impl<'a, I> QNameMinimizer<'a, I>
where
    I: 'a + Iterator<Item = CDomainName> + ExactSizeIterator + DoubleEndedIterator,
{
    pub fn new_limited_minimizer(
        qname: &'a CDomainName,
        search_names: I,
        qname_minimization_limit: usize,
    ) -> Self {
        Self::LimitedMinimizer {
            qname,
            remaining_minimized_qnames: qname_minimization_limit,
            qname_iter: search_names,
        }
    }

    pub fn new_repeater(qname: &'a CDomainName, skip: usize) -> Self {
        Self::None {
            qname,
            repeat_n: qname.label_count().saturating_sub(skip),
        }
    }
}

impl<'a, I> Iterator for QNameMinimizer<'a, I>
where
    I: 'a + Iterator<Item = CDomainName> + ExactSizeIterator + DoubleEndedIterator,
{
    type Item = CDomainName;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            QNameMinimizer::None {
                qname: _,
                repeat_n: 0,
            } => None,
            QNameMinimizer::None { qname, repeat_n } => {
                *repeat_n -= 1;
                Some(qname.clone())
            }
            QNameMinimizer::LimitedMinimizer {
                qname,
                remaining_minimized_qnames: 0,
                qname_iter,
            } => {
                // When we have reached the minimization limit, transition to
                // an iterator that only outputs the full domain name.
                let remaining_iterations = qname_iter.len();
                if remaining_iterations == 0 {
                    *self = QNameMinimizer::None {
                        qname,
                        repeat_n: remaining_iterations,
                    };
                    None
                } else {
                    let returned_qname = qname.clone();
                    *self = QNameMinimizer::None {
                        qname,
                        repeat_n: remaining_iterations - 1,
                    };
                    Some(returned_qname)
                }
            }
            QNameMinimizer::LimitedMinimizer {
                qname: _,
                remaining_minimized_qnames,
                qname_iter,
            } => {
                *remaining_minimized_qnames -= 1;
                qname_iter.next()
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            QNameMinimizer::None { qname: _, repeat_n } => (*repeat_n, Some(*repeat_n)),
            QNameMinimizer::LimitedMinimizer {
                qname: _,
                remaining_minimized_qnames: _,
                qname_iter,
            } => qname_iter.size_hint(),
        }
    }
}

impl<'a, I> ExactSizeIterator for QNameMinimizer<'a, I>
where
    I: 'a + Iterator<Item = CDomainName> + ExactSizeIterator + DoubleEndedIterator,
{
    fn len(&self) -> usize {
        match self {
            QNameMinimizer::None { qname: _, repeat_n } => *repeat_n,
            QNameMinimizer::LimitedMinimizer {
                qname: _,
                remaining_minimized_qnames: _,
                qname_iter,
            } => qname_iter.len(),
        }
    }
}

impl<'a, I> DoubleEndedIterator for QNameMinimizer<'a, I>
where
    I: 'a + Iterator<Item = CDomainName> + ExactSizeIterator + DoubleEndedIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            QNameMinimizer::None {
                qname: _,
                repeat_n: 0,
            } => None,
            QNameMinimizer::None { qname, repeat_n } => {
                *repeat_n -= 1;
                Some(qname.clone())
            }
            QNameMinimizer::LimitedMinimizer {
                qname,
                remaining_minimized_qnames: 0,
                qname_iter,
            } => {
                // When we have reached the minimization limit, transition to
                // an iterator that only outputs the full domain name.
                let remaining_iterations = qname_iter.len();
                if remaining_iterations == 0 {
                    *self = QNameMinimizer::None {
                        qname,
                        repeat_n: remaining_iterations,
                    };
                    None
                } else {
                    let returned_qname = qname.clone();
                    *self = QNameMinimizer::None {
                        qname,
                        repeat_n: remaining_iterations - 1,
                    };
                    Some(returned_qname)
                }
            }
            QNameMinimizer::LimitedMinimizer {
                qname: _,
                remaining_minimized_qnames,
                qname_iter,
            } => {
                *remaining_minimized_qnames -= 1;
                qname_iter.next_back()
            }
        }
    }
}
