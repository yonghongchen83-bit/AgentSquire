# Task 012: Todo Tree Tool — crash reproduction

## Description
The built-in `todo_tree` tool causes the app to crash/restart when used in a Legacy chat session.

## Steps
1. Launch the app
2. Verify the app loads without console errors
3. List available tools — verify `todo_tree` is registered
4. Send a chat message: "use todo tree to create a plan of downloading books from a given website and make a html for local reading"
5. The app must NOT crash or restart

## Expected Results
- App loads without critical errors
- `todo_tree` appears in available tools
- Chat message is sent without causing the app to crash
- No browser console errors
