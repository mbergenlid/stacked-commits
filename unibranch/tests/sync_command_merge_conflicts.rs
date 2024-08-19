use git2::Oid;
use indoc::formatdoc;
use pretty_assertions::assert_eq;
use test_repo::{RemoteRepo, TestRepoWithRemote};
use ubr::{
    commands::{create, sync},
    git::GitRepo,
};

fn git_repo(value: &TestRepoWithRemote) -> GitRepo {
    GitRepo::open(value.local_repo_dir.path()).unwrap()
}

fn push_options(commit_ref: Option<Oid>) -> create::Options {
    create::Options {
        commit_ref: commit_ref.map(|id| format!("{}", id)),
        force: false,
    }
}

#[test]
fn test_merge_conflict_from_remote() {
    let remote_repo = RemoteRepo::new();
    let local_repo = remote_repo
        .clone_repo()
        .create_file("File1", "Hello, World!")
        .commit_all("commit1")
        .push()
        .append_file("File1", "Starting on a new feature")
        .commit_all("feature 1");

    //Create a PR from local repo
    create::execute(
        create::Options {
            commit_ref: Some("HEAD".to_string()),
            force: false,
        },
        git_repo(&local_repo),
    )
    .expect("Unable to create initial PR");

    let remote_head = {
        let another_local_clone = remote_repo.clone_repo();

        another_local_clone
            .checkout("feature-1")
            .append_file("File1", "Some remote fixes")
            .commit_all("Fixup")
            .push()
            .head()
    };

    let local_repo = local_repo
        .append_file("File1", "Some local fixes")
        .commit_all_amend();

    let result = sync::execute(sync::Options::default(), git_repo(&local_repo));
    assert!(result.is_err());
    let expected_error_message = formatdoc! {"
        Unable to merge local commit ({local}) with commit from remote ({remote})
        Once all the conflicts has been resolved, run 'ubr sync --continue'
        ",
        local = local_repo.head(),
        remote = remote_head
    };
    assert_eq!(format!("{}", result.unwrap_err()), expected_error_message);

    let merge_head = String::from_utf8(
        std::fs::read(
            local_repo
                .local_repo_dir
                .path()
                .join(".ubr/SYNC_MERGE_HEAD"),
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(merge_head, format!("{}\n", remote_head));

    let local_repo = local_repo
        .create_file(
            "File1",
            "Starting on a new feature\nSome local/remote fixes",
        )
        .add_all();

    {
        let resolved_file = String::from_utf8(
            std::fs::read(local_repo.local_repo_dir.path().join("File1")).unwrap(),
        )
        .unwrap();

        assert_eq!(
            resolved_file,
            "Starting on a new feature\nSome local/remote fixes\n"
        );
    }

    sync::execute(sync::Options { cont: true }, git_repo(&local_repo)).expect("Should succeed");
}
