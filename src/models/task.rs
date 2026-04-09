use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_str(&self) -> &str {
        match self {
            TaskPriority::Low => "low",
            TaskPriority::Medium => "medium",
            TaskPriority::High => "high",
            TaskPriority::Critical => "critical",
        }
    }

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().as_str() {
            "low" => Ok(TaskPriority::Low),
            "medium" => Ok(TaskPriority::Medium),
            "high" => Ok(TaskPriority::High),
            "critical" => Ok(TaskPriority::Critical),
            _ => Err(anyhow::anyhow!("Unknown priority: '{}'. Use: low, medium, high, critical", s)),
        }
    }

    pub fn sort_order(&self) -> u8 {
        match self {
            TaskPriority::Critical => 0,
            TaskPriority::High => 1,
            TaskPriority::Medium => 2,
            TaskPriority::Low => 3,
        }
    }
}

impl fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for TaskPriority {
    fn default() -> Self {
        TaskPriority::Medium
    }
}

/// Lifecycle status of a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Blocked,
    Done,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Todo => "todo",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Blocked => "blocked",
            TaskStatus::Done => "done",
            TaskStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> anyhow::Result<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            "todo" => Ok(TaskStatus::Todo),
            "in_progress" | "inprogress" => Ok(TaskStatus::InProgress),
            "blocked" => Ok(TaskStatus::Blocked),
            "done" | "completed" => Ok(TaskStatus::Done),
            "cancelled" | "canceled" => Ok(TaskStatus::Cancelled),
            _ => Err(anyhow::anyhow!("Unknown status: '{}'. Use: todo, in_progress, blocked, done, cancelled", s)),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Done | TaskStatus::Cancelled)
    }
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Todo
    }
}

/// Core Task entity — the primary domain object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub project_id: Option<String>,
    pub assignee: Option<String>,
    pub tags: Vec<String>,
    pub due_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Business rule: mark task as done
    pub fn complete(&mut self) {
        self.status = TaskStatus::Done;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Business rule: check if task is overdue
    pub fn is_overdue(&self) -> bool {
        if self.status.is_terminal() {
            return false;
        }
        self.due_date
            .map(|due| due < Utc::now())
            .unwrap_or(false)
    }

    /// Days until due (negative = overdue)
    pub fn days_until_due(&self) -> Option<i64> {
        self.due_date.map(|due| {
            let diff = due.signed_duration_since(Utc::now());
            diff.num_days()
        })
    }
}

/// Request payload for creating a new task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<TaskPriority>,
    pub project_id: Option<String>,
    pub assignee: Option<String>,
    pub tags: Option<Vec<String>>,
    pub due_date: Option<DateTime<Utc>>,
}

impl CreateTaskRequest {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.title.trim().is_empty() {
            return Err(anyhow::anyhow!("Task title cannot be empty"));
        }
        if self.title.len() > 255 {
            return Err(anyhow::anyhow!("Task title must be 255 characters or fewer"));
        }
        Ok(())
    }
}

/// Request payload for updating an existing task
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub project_id: Option<String>,
    pub assignee: Option<String>,
    pub tags: Option<Vec<String>>,
    pub due_date: Option<DateTime<Utc>>,
}

/// Filter parameters for listing tasks
#[derive(Debug, Clone, Default)]
pub struct TaskFilter {
    pub status: Option<TaskStatus>,
    pub priority: Option<TaskPriority>,
    pub project_id: Option<String>,
    pub assignee: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
    pub overdue_only: bool,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}
