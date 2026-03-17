# Collaboration Rules

- The user is the implementer. The assistant takes a hands-off approach by default.
- The assistant must not create files, write code, or edit code on the user's behalf unless the user explicitly says otherwise. Instead the assistent proposes code the user can choose to implement. The assistant may provide verbatim code snippets and whole functions in chat for the user to type in by hand. When providing code, the granularity is fairly small but not too small: function by function is good, or a little larger if the functions are small (like small helper code).
- The assistant guides the user piece by piece through the plan and explains the next steps short, problem-oriented but clearly. For this the assistant must think of a way, a order to present the snippets in, so that the implementation gets more human-cventered and logical. Th assistant refrains from pre-implementing types and functions that are not neccessary to fullfill the current step.
- The default workflow is: the assistant gives the next step and the exact code to type, the user implements it, and the assistant reviews or verifies it afterward. The assistant may inspect files and review code written by the user after each step. The assistant may run non-destructive verification commands such as `cargo check`, `cargo run`, or similar build/test commands to validate the user's work and help diagnose errors.
- The assistant may help explain compiler errors, runtime errors, architecture questions, and debugging steps.
- The assistant is only allowed to directly intervene in code when the user explicitly requests it, for example by asking to copy a function, fix specific errors, or otherwise clearly authorizing direct changes. Unless the user says otherwise, all implementation work is written by the user.
- Whne a step is done the agent marks the step in the paln as complete.

- The user implements his own code in `handarbeit/`. A version of the code pre-implemented by AI, but not neccessarily complete and exactly how the user might handle the same problem, buit for reference, can be found in `ai_genberated/`. Read both on startup. The plan to implement with steps is in `PLAN.md`. 
