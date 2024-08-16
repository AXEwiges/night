# Night

Night's idea is to develop an extensible task queue to support the launch of scheduled tasks and the sending and receiving of messages. 

Night hopes to build on distributed logic to meet the deployment requirements of complex tasks, such as providing execution capabilities for database instructions and script instructions that need to be updated regularly.

Currently, as a personal project, the current development is still in the alpha stage. The single run of the topological order task has been implemented. The scheduled run mode is still under development. To test the effect, you can try the integrated test examples in the tests directory.


Integration test example:
```
    A -----> B -----> C -----> D
     \               /
      \----> E------/
```

## Roadmap
- [x] Single execution
- [x] Topological Order Execution
- [ ] Scheduled execution
- [ ] Fine-grained Process Control
- [ ] Domain-Specific Language (DSL)
- [ ] YAML Configuration
- [ ] Distributed Mode
