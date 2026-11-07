use cohesion_core::model::{ImageBytes, RemoteChain, ThreadItem, ThreadOrigin};

use crate::test_support::context;
use crate::{
    CreateIntelligenceMessageCommand, CreateIntelligenceThreadCommand, DatabaseContext,
    ForkIntelligenceThreadCommand, GetIntelligenceThreadMessagesCommand, ThreadEntry,
    UpdateIntelligenceThreadRemoteCommand,
};

/// A thread holding messages `"0"` to `"<count - 1>"`, whose stored chain
/// ends after `cursor` of them.
fn thread(db: &DatabaseContext, count: usize, cursor: i64) -> i64 {
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "You are Scout.".to_string(),
        })
        .unwrap();
    for index in 0..count {
        db.send_blocking(CreateIntelligenceMessageCommand {
            thread_id: thread.id,
            item: ThreadItem::UserMessage {
                text: index.to_string(),
            },
            images: Vec::new(),
        })
        .unwrap();
    }
    db.send_blocking(UpdateIntelligenceThreadRemoteCommand {
        thread_id: thread.id,
        remote: Some(RemoteChain {
            remote_id: "resp_1".to_string(),
            remote_cursor: cursor,
        }),
    })
    .unwrap();
    thread.id
}

fn text(entry: &ThreadEntry) -> &str {
    match &entry.message.item {
        ThreadItem::UserMessage { text } => text,
        other => panic!("unexpected item {other:?}"),
    }
}

fn say(db: &DatabaseContext, thread_id: i64, said: &str) {
    db.send_blocking(CreateIntelligenceMessageCommand {
        thread_id,
        item: ThreadItem::UserMessage {
            text: said.to_string(),
        },
        images: Vec::new(),
    })
    .unwrap();
}

fn read(db: &DatabaseContext, thread_id: i64) -> Vec<String> {
    db.send_blocking(GetIntelligenceThreadMessagesCommand { thread_id })
        .unwrap()
        .iter()
        .map(|entry| text(entry).to_string())
        .collect()
}

#[test]
fn a_fork_inherits_the_first_messages_and_the_chain_that_ends_in_them() {
    let db = context();
    let source = thread(&db, 4, 2);

    let (fork, inherited) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: source,
            items: 3,
        })
        .unwrap();

    assert_ne!(fork.id, source);
    assert_eq!(fork.system_prompt, "You are Scout.");
    assert_eq!(
        inherited.iter().map(text).collect::<Vec<_>>(),
        ["0", "1", "2"]
    );
    // Nothing is copied: the messages stay the source's.
    assert!(
        inherited
            .iter()
            .all(|entry| entry.message.thread_id == source)
    );
    assert!(matches!(
        fork.origin,
        ThreadOrigin::Fork { parent_thread_id, inherited_items: 3, .. } if parent_thread_id == source
    ));
    assert_eq!(fork.remote.unwrap().remote_cursor, 2);
}

#[test]
fn a_fork_that_stops_before_the_chain_ends_starts_its_own() {
    let db = context();
    let source = thread(&db, 4, 4);

    let (fork, inherited) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: source,
            items: 3,
        })
        .unwrap();

    assert_eq!(inherited.len(), 3);
    assert_eq!(fork.remote, None);
}

#[test]
fn a_fork_reads_what_it_inherits_then_its_own_and_not_what_its_parent_says_later() {
    let db = context();
    let source = thread(&db, 2, 0);
    let (fork, _) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: source,
            items: 2,
        })
        .unwrap();

    say(&db, source, "parent later");
    say(&db, fork.id, "fork");

    assert_eq!(read(&db, fork.id), ["0", "1", "fork"]);
    assert_eq!(read(&db, source), ["0", "1", "parent later"]);
}

#[test]
fn a_fork_of_a_fork_inherits_from_whichever_thread_holds_its_last_message() {
    let db = context();
    let root = thread(&db, 3, 0);
    let (first, _) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: root,
            items: 3,
        })
        .unwrap();
    say(&db, first.id, "first");

    let (through_own, _) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: first.id,
            items: 4,
        })
        .unwrap();
    assert_eq!(read(&db, through_own.id), ["0", "1", "2", "first"]);
    assert!(matches!(
        through_own.origin,
        ThreadOrigin::Fork { parent_thread_id, .. } if parent_thread_id == first.id
    ));

    let (before_own, _) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: first.id,
            items: 2,
        })
        .unwrap();
    assert_eq!(read(&db, before_own.id), ["0", "1"]);
    assert!(matches!(
        before_own.origin,
        ThreadOrigin::Fork { parent_thread_id, .. } if parent_thread_id == root
    ));
}

#[test]
fn a_fork_of_nothing_is_a_root_and_threads_share_a_prompt() {
    let db = context();
    let source = thread(&db, 1, 0);
    let (fork, _) = db
        .send_blocking(ForkIntelligenceThreadCommand {
            thread_id: source,
            items: 0,
        })
        .unwrap();
    assert_eq!(fork.origin, ThreadOrigin::Root);
    let other = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: "You are Scout.".to_string(),
        })
        .unwrap();
    assert_eq!(other.prompt_id, fork.prompt_id);
}

#[test]
fn an_image_comes_back_with_its_message() {
    let db = context();
    let thread = db
        .send_blocking(CreateIntelligenceThreadCommand {
            system_prompt: String::new(),
        })
        .unwrap();
    db.send_blocking(CreateIntelligenceMessageCommand {
        thread_id: thread.id,
        item: ThreadItem::UserMessage {
            text: "Look".to_string(),
        },
        images: vec![(
            "image/png".to_string(),
            ImageBytes::from_base64("AAAA").unwrap(),
        )],
    })
    .unwrap();
    let entries = db
        .send_blocking(GetIntelligenceThreadMessagesCommand {
            thread_id: thread.id,
        })
        .unwrap();
    assert_eq!(entries[0].images[0].data.to_base64(), "AAAA");
}
