#[derive(Clone)]
pub struct Task {
    pub title: String,
    pub done: bool,
}

/// Pure application state: no Slint, window, or widget dependency.
pub struct TaskStore {
    tasks: Vec<Task>,
}

impl TaskStore {
    pub fn demo() -> Self {
        Self {
            tasks: vec![
                Task {
                    title: "Explore Slint markup".into(),
                    done: true,
                },
                Task {
                    title: "Keep business logic in Rust".into(),
                    done: false,
                },
                Task {
                    title: "Ship a native release build".into(),
                    done: false,
                },
            ],
        }
    }

    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    pub fn add(&mut self, title: impl Into<String>) {
        let title = title.into().trim().to_owned();
        if !title.is_empty() {
            self.tasks.push(Task { title, done: false });
        }
    }

    pub fn toggle(&mut self, index: usize) {
        if let Some(task) = self.tasks.get_mut(index) {
            task.done = !task.done;
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.tasks.len() {
            self.tasks.remove(index);
        }
    }

    pub fn completed_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.done).count()
    }
}
