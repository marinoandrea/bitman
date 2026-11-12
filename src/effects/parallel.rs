//! Safe scoped chunks. Only scheduling creates allocations; serial kernels do not.

#[derive(Clone, Copy, Debug)]
pub struct Execution {
    pub threads: usize,
    pub min_chunk: usize,
}

impl Default for Execution {
    fn default() -> Self {
        Self {
            threads: std::thread::available_parallelism()
                .map_or(1, usize::from)
                .min(8),
            min_chunk: 262_144,
        }
    }
}

impl Execution {
    pub const SERIAL: Self = Self {
        threads: 1,
        min_chunk: usize::MAX,
    };

    pub fn worker_count(&self, len: usize, alignment: usize) -> usize {
        let units = len / alignment;
        self.threads
            .clamp(1, 8)
            .min(len.div_ceil(self.min_chunk.max(1)))
            .min(units)
            .max(1)
    }

    pub fn chunks<T: Send>(
        &self,
        output: &mut [T],
        alignment: usize,
        f: impl Fn(usize, &mut [T]) + Sync,
    ) {
        let units = output.len() / alignment;
        let workers = self.worker_count(output.len(), alignment);
        if workers == 1 {
            f(0, output);
            return;
        }
        let chunk = units.div_ceil(workers) * alignment;
        std::thread::scope(|scope| {
            let f = &f;
            for (i, part) in output.chunks_mut(chunk).enumerate() {
                scope.spawn(move || f(i * chunk, part));
            }
        });
    }
}
