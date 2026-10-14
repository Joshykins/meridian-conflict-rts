use mc_sim::mission::{Action, Condition, Event};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Default)]
pub(crate) struct Script {
    events: Rc<RefCell<Vec<Event>>>,
    last: Rc<RefCell<Option<String>>>,
}

impl Script {
    pub(crate) fn event(&self, name: &str, when: Condition, actions: Vec<Action>) {
        self.events.borrow_mut().push(Event {
            name: name.into(),
            when,
            actions,
        });
    }
    pub(crate) fn then(&self, name: &str, condition: Condition, actions: Vec<Action>) {
        let when = match self.last.borrow().as_ref() {
            Some(previous) => Condition::All(vec![
                Condition::After {
                    event: previous.clone(),
                    ticks: 0,
                },
                condition,
            ]),
            None => condition,
        };
        self.event(name, when, actions);
        *self.last.borrow_mut() = Some(name.into());
    }
    pub(crate) async fn wait(&self, name: &str, condition: Condition) {
        self.then(name, condition, vec![]);
    }
    pub(crate) async fn say(&self, line: &str, ticks: u32) {
        self.then(
            line,
            Condition::Elapsed(0),
            vec![Action::Say {
                line: line.into(),
                ticks,
            }],
        );
        self.wait(
            &format!("{line}.end"),
            Condition::After {
                event: line.into(),
                ticks,
            },
        )
        .await;
    }
    pub(crate) fn finish(self) -> Vec<Event> {
        self.events.borrow().clone()
    }
}
