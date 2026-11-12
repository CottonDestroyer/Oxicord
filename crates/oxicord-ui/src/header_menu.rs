//! The menu under the server name at the top of the channel list: which rows
//! it holds for this person in this server, and how they are grouped. The
//! drawing lives in `context_menu`.

/// One row of the menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Boost,
    Tag,
    Invite,
    Settings,
    CreateEvent,
    AppDirectory,
    Notifications,
    Privacy,
    Profile,
    HideMuted,
    Leave,
    CopyId,
}

impl Row {
    pub fn label(self) -> &'static str {
        match self {
            Row::Boost => "Server Boost",
            Row::Tag => "Server Tag",
            Row::Invite => "Invite to Server",
            Row::Settings => "Server Settings",
            Row::CreateEvent => "Create Event",
            Row::AppDirectory => "App Directory",
            Row::Notifications => "Notification Settings",
            Row::Privacy => "Privacy Settings",
            Row::Profile => "Edit Per-server Profile",
            Row::HideMuted => "Hide Muted Channels",
            Row::Leave => "Leave Server",
            Row::CopyId => "Copy Server ID",
        }
    }

    /// The picture left of the label. The tag row draws its chip instead.
    pub fn glyph(self) -> &'static str {
        match self {
            Row::Boost => "server-boost",
            Row::Tag => "",
            Row::Invite => "people-add",
            Row::Settings => "settings",
            Row::CreateEvent => "calendar-plus",
            Row::AppDirectory => "layout-grid",
            Row::Notifications => "bell",
            Row::Privacy => "shield",
            Row::Profile => "pencil",
            Row::HideMuted => "eye-off",
            Row::Leave => "log-out",
            Row::CopyId => "id-badge",
        }
    }
}

/// What decides which rows show.
#[derive(Clone, Copy, Debug, Default)]
pub struct Facts {
    /// The server hands out a tag.
    pub tag: bool,
    /// This person may change the server's settings.
    pub manage: bool,
    /// This person may create events.
    pub events: bool,
    /// This person owns the server, who can't leave it.
    pub owner: bool,
}

/// The rows in the groups the live menu draws, a line between groups.
pub fn groups(facts: Facts) -> Vec<Vec<Row>> {
    let mut first = vec![Row::Boost];
    if facts.tag {
        first.push(Row::Tag);
    }
    let mut second = vec![Row::Invite];
    if facts.manage {
        second.push(Row::Settings);
    }
    if facts.events {
        second.push(Row::CreateEvent);
    }
    second.push(Row::AppDirectory);
    let mut groups = vec![
        first,
        second,
        vec![Row::Notifications, Row::Privacy],
        vec![Row::Profile, Row::HideMuted],
    ];
    if !facts.owner {
        groups.push(vec![Row::Leave]);
    }
    groups.push(vec![Row::CopyId]);
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_member_gets_the_short_menu() {
        let rows = groups(Facts::default());
        assert_eq!(
            rows,
            vec![
                vec![Row::Boost],
                vec![Row::Invite, Row::AppDirectory],
                vec![Row::Notifications, Row::Privacy],
                vec![Row::Profile, Row::HideMuted],
                vec![Row::Leave],
                vec![Row::CopyId],
            ]
        );
    }

    #[test]
    fn an_admin_with_a_tagged_server_gets_every_row_in_order() {
        let rows = groups(Facts { tag: true, manage: true, events: true, owner: false });
        let flat: Vec<Row> = rows.into_iter().flatten().collect();
        assert_eq!(
            flat,
            [
                Row::Boost,
                Row::Tag,
                Row::Invite,
                Row::Settings,
                Row::CreateEvent,
                Row::AppDirectory,
                Row::Notifications,
                Row::Privacy,
                Row::Profile,
                Row::HideMuted,
                Row::Leave,
                Row::CopyId,
            ]
        );
    }

    #[test]
    fn the_owner_cannot_leave_so_the_row_and_its_line_go() {
        let rows = groups(Facts { owner: true, ..Facts::default() });
        assert!(rows.iter().flatten().all(|row| *row != Row::Leave));
        assert_eq!(rows.last(), Some(&vec![Row::CopyId]));
        assert!(rows.iter().all(|group| !group.is_empty()));
    }
}
