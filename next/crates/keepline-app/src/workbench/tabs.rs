//! Terminal tabs: their shown name and renaming.
use super::*;

impl Workbench {
    /// Program titles come from the daemon, which reads every terminal, so tabs in
    /// the back keep theirs. The focused pane's title wins over its siblings'.
    pub(super) fn tab_name(&self, tab: &Tab, _cx: &gpui::App) -> String {
        self.workspace.tab_title(
            tab,
            &self.store.recognition().terminal_titles,
            self.nav.focused.as_deref(),
        )
    }
    /// Double-click names a tab; an empty name hands it back to its contents.
    pub(super) fn rename_tab(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.workspace.tabs.iter().find(|t| t.id == id) else {
            return;
        };
        let current = self.tab_name(tab, cx);
        let field = cx.new(|cx| Field::new("标签名（回车保存，Esc 取消）", &current, cx).compact());
        field.update(cx, |f, _| f.select_all());
        let subscription = cx.subscribe(&field, |this, _, event, cx| match event {
            crate::ui::field::FieldEvent::Submit => this.save_tab_name(cx),
            crate::ui::field::FieldEvent::Cancel => {
                this.layout.renaming_tab = None;
                cx.notify();
            }
        });
        window.focus(&field.focus_handle(cx), cx);
        self.layout.renaming_tab = Some((id.to_owned(), field, subscription));
        cx.notify();
    }
    fn save_tab_name(&mut self, cx: &mut Context<Self>) {
        let Some((id, field, _)) = self.layout.renaming_tab.take() else {
            return;
        };
        let Some(mut tab) = self.workspace.tabs.iter().find(|t| t.id == id).cloned() else {
            return;
        };
        let name = field.read(cx).text.trim().to_owned();
        // Keeping the shown name unchanged must not pin today's automatic title.
        tab.title = if tab.title.is_empty() && name == self.tab_name(&tab, cx) {
            String::new()
        } else {
            name
        };
        self.edit(Edit::PutTab { tab }, cx);
        cx.notify();
    }
}
