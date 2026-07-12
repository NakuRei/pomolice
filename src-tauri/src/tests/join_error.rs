pub(crate) fn join_error_from_panic(message: &'static str) -> tokio::task::JoinError {
    let Ok(runtime) = tokio::runtime::Runtime::new() else {
        panic!("test runtime should be created");
    };

    runtime.block_on(async move {
        let join_result = tokio::spawn(async move {
            panic!("{message}");
        })
        .await;
        let Err(join_error) = join_result else {
            panic!("spawned panic should surface as join error");
        };

        join_error
    })
}
