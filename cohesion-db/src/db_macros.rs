//! Every `macro_rules!` of this crate.

/// Every model, so a new one is checked by adding it here once. The names
/// resolve where it is invoked, which imports each model.
macro_rules! every_model {
    ($check:ident($db:expr)) => {
        $check::<Workspace>($db);
        $check::<Document>($db);
        $check::<DocumentFolder>($db);
        $check::<Client>($db);
        $check::<SecuritySettings>($db);
        $check::<SecurityEvent>($db);
        $check::<StorageSettings>($db);
        $check::<EnabledExperiment>($db);
        $check::<Profile>($db);
        $check::<TaskFolder>($db);
        $check::<Task>($db);
        $check::<CatalogModel>($db);
        $check::<IntelligenceEndpoint>($db);
        $check::<EndpointModel>($db);
        $check::<IntelligenceDefault>($db);
        $check::<IntelligenceSpendLimit>($db);
        $check::<IntelligenceCostEvent>($db);
        $check::<ThreadPrompt>($db);
        $check::<IntelligenceThread>($db);
        $check::<IntelligenceThreadMessage>($db);
        $check::<IntelligenceThreadImage>($db);
        $check::<IntelligenceAgent>($db);
        $check::<IntelligenceAgentCue>($db);
        $check::<AgentAgendaItem>($db);
        $check::<AgentSkill>($db);
        $check::<IntelligenceAgentTeam>($db);
        $check::<IntelligenceAgentMessage>($db);
        $check::<IntelligenceAgentRun>($db);
        $check::<Actor>($db);
        $check::<ActorSession>($db);
        $check::<ActorSessionMessage>($db);
        $check::<Attachment>($db);
        $check::<Memory>($db);
        $check::<Extension>($db);
        $check::<ExtensionPermission>($db);
        $check::<ExtensionHook>($db);
        $check::<ExtensionService>($db);
        $check::<ExtensionServiceFunction>($db);
        $check::<Metadata>($db);
        $check::<ConnectorAccount>($db);
        $check::<Connector>($db);
        $check::<ResourceKind>($db);
        $check::<Resource>($db);
        $check::<WorkspaceResourceKind>($db);
        $check::<Feed>($db);
        $check::<Article>($db);
        $check::<StorageEntry>($db);
        $check::<Contact>($db);
        $check::<ContactMethod>($db);
        $check::<CalendarStore>($db);
        $check::<Calendar>($db);
        $check::<CalendarEvent>($db);
        $check::<Tag>($db);
        $check::<TaskTag>($db);
        $check::<SavedTaskView>($db);
        $check::<SavedTaskViewTag>($db);
        $check::<SavedTaskViewPriority>($db);
        $check::<SavedTaskViewSize>($db);
        $check::<AgentThreadMark>($db);
        $check::<RewindWriter>($db);
        $check::<Version>($db);
        $check::<Backlink>($db);
        $check::<DocumentFolderProperty>($db);
        $check::<DocumentValue>($db);
        $check::<PropertyOption>($db);
    };
}
pub(crate) use every_model;
