use super::*;
use mqtt_bridge_kit::*;

pub struct DaliProxy {
    pub(super) handle: ServiceHandle,
    pub(super) initial: TargetGenerations,
    pub(super) reporter: Option<Reporter>,
}
impl DeviceDriver for DaliProxy {
    fn info(&self) -> DriverInfo {
        DriverInfo {
            commands: ["rename", "rename_bus", "describe", "recover"]
                .into_iter()
                .map(|name| CommandSpec::new(name, CommandClass::WithReply))
                .collect(),
            ..DriverInfo::default()
        }
    }
    fn request_policy(&self) -> RequestPolicy {
        RequestPolicy::Fenced(self.initial.clone())
    }
    fn validate_request(
        &self,
        target: Option<&str>,
        context: &RequestContext,
        kind: RequestKind<'_>,
    ) -> Result<(), OpError> {
        if target == Some("__inventory") {
            return Err(OpError::Unsupported("inventory is read-only".into()));
        }
        match kind {
            RequestKind::Command(call) => {
                let snapshot = self
                    .handle
                    .snapshot()
                    .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
                let change = parse(&snapshot, target, call)?;
                routing::admit(&snapshot, &change, context).map_err(op_error)
            }
            _ => Err(OpError::Unsupported(
                "hardware properties and target configuration pending".into(),
            )),
        }
    }
    async fn connect(&mut self, ctx: &mut LinkContext) -> Result<(), LinkError> {
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| LinkError::new("service is starting"))?;
        if ctx.instance() != snapshot.model.inventory().name {
            return Err(LinkError::new(
                "runtime instance must match persisted controller name",
            ));
        }
        self.handle
            .attach(ctx.report.clone())
            .await
            .map_err(LinkError::new)?;
        self.reporter = Some(ctx.report.clone());
        Ok(())
    }
    async fn disconnect(&mut self) {
        self.reporter = None;
    }
    async fn read_state(&mut self, _scope: Scope) -> Result<Vec<TargetState>, OpError> {
        Err(OpError::Unsupported("context required".into()))
    }
    async fn read_state_with_context(
        &mut self,
        scope: Scope,
    ) -> Result<Vec<ContextualState>, OpError> {
        if matches!(&scope,Scope::Target(target) if target!="__inventory") {
            return Err(OpError::Unsupported("hardware readback pending".into()));
        }
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
        if matches!(snapshot.phase, Phase::Stopping | Phase::Closed) {
            return Err(OpError::Link(LinkError::new("service stopped")));
        }
        let context = RequestContext::new(routing::generations(&snapshot.model).instance)
            .map_err(OpError::Unusable)?;
        let values =
            serde_json::from_slice(&snapshot.model.state_bytes().map_err(OpError::Unusable)?)
                .map_err(|e| OpError::Unusable(e.to_string()))?;
        Ok(vec![ContextualState::new(
            TargetState::new("__inventory", values),
            context,
        )])
    }
    async fn apply(&mut self, _target: &str, _values: &PropertyMap) -> Result<(), OpError> {
        Err(OpError::Unsupported("hardware properties pending".into()))
    }
    async fn execute(
        &mut self,
        _target: Option<&str>,
        _call: &CommandCall,
    ) -> Result<Option<Value>, OpError> {
        Err(OpError::Rejected(
            "original request context required".into(),
        ))
    }
    async fn execute_with_context(
        &mut self,
        target: Option<&str>,
        call: &CommandCall,
        context: &RequestContext,
    ) -> Result<Option<Value>, OpError> {
        let snapshot = self
            .handle
            .snapshot()
            .ok_or_else(|| OpError::Rejected("service is starting".into()))?;
        let change = parse(&snapshot, target, call)?;
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        let completion = self
            .reporter
            .as_ref()
            .and_then(Reporter::command_completion)
            .ok_or_else(|| OpError::Rejected("active command completion is required".into()))?;
        let result = self
            .handle
            .submit(change, context.clone(), deadline, Some(completion))
            .await
            .map_err(op_error)?;
        Ok(Some(
            serde_json::json!({"revision":result.model.inventory().revision}),
        ))
    }
    async fn configure(&mut self, _target: &str, _config: Option<&Value>) -> Result<(), OpError> {
        Err(OpError::Unsupported("target configuration pending".into()))
    }
}

fn op_error(error: ServiceError) -> OpError {
    OpError::Rejected(error.to_string())
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Rename {
    expected_revision: u64,
    name: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameBus {
    expected_revision: u64,
    bus: u8,
    name: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Describe {
    expected_revision: u64,
    description: Option<String>,
}
fn decode<T: serde::de::DeserializeOwned>(call: &CommandCall) -> Result<T, OpError> {
    serde_json::from_value(Value::Object(call.params.clone()))
        .map_err(|e| OpError::Rejected(e.to_string()))
}
fn parse(snapshot: &Snapshot, target: Option<&str>, call: &CommandCall) -> Result<Change, OpError> {
    let edit = match (call.command.as_str(), target) {
        ("rename", Some(target)) if target != "__inventory" => {
            let params: Rename = decode(call)?;
            let projection = snapshot
                .model
                .projections()
                .into_iter()
                .find(|p| p.target == target)
                .ok_or_else(|| OpError::Rejected("unknown item".into()))?;
            let edit = match projection.location {
                crate::inventory::Location::Output { bus, short_address } => {
                    MetadataEdit::RenameOutput {
                        bus,
                        address: short_address,
                        name: params.name,
                    }
                }
                crate::inventory::Location::Group { bus, group_address } => {
                    MetadataEdit::RenameGroup {
                        bus,
                        address: group_address,
                        name: params.name,
                    }
                }
            };
            (params.expected_revision, edit)
        }
        ("rename_bus", None) => {
            let params: RenameBus = decode(call)?;
            (
                params.expected_revision,
                MetadataEdit::RenameBus {
                    bus: params.bus,
                    name: params.name,
                },
            )
        }
        ("describe", None) => {
            let params: Describe = decode(call)?;
            if !call.params.contains_key("description") {
                return Err(OpError::Rejected(
                    "description is required; use null to clear".into(),
                ));
            }
            (
                params.expected_revision,
                MetadataEdit::Description(params.description),
            )
        }
        ("recover", None) if call.params.is_empty() => return Ok(Change::Recover),
        _ => return Err(OpError::Unsupported("unsupported command or target".into())),
    };
    Ok(Change::Edit {
        expected: edit.0,
        edit: edit.1,
    })
}
