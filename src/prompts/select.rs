use std::{io, ops::Rem};
use std::io::{Error, ErrorKind};
use console::{Key, Term};
use InterruptableResult::{AccessRevoked, AsRequested, IOError, WriteLine};
use LoopStatement::Break;
use crate::{
    theme::{render::TermThemeRenderer, SimpleTheme, Theme},
    Paging, Result,
};
use crate::prompts::select::LoopStatement::{Continue, Return};
use crate::term_access::{InterruptableResult, TermAccess};

/// Renders a select prompt.
///
/// User can select from one or more options.
/// Interaction returns index of an item selected in the order they appear in `item` invocation or `items` slice.
///
/// ## Example
///
/// ```rust,no_run
/// use dialoguer::Select;
///
/// fn main() {
///     let items = vec!["foo", "bar", "baz"];
///
///     let selection = Select::new()
///         .with_prompt("What do you choose?")
///         .items(&items)
///         .interact()
///         .unwrap();
///
///     println!("You chose: {}", items[selection]);
/// }
/// ```
#[derive(Clone)]
pub struct Select<'a> {
    default: usize,
    items: Vec<String>,
    prompt: Option<String>,
    report: bool,
    clear: bool,
    theme: &'a dyn Theme,
    max_length: Option<usize>,
}

impl Default for Select<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl Select<'static> {
    /// Creates a select prompt with default theme.
    pub fn new() -> Self {
        Self::with_theme(&SimpleTheme)
    }
}

enum LoopStatement<T> {
    Continue,
    Break,
    Return(T)
}

impl Select<'_> {
    /// Indicates whether select menu should be erased from the screen after interaction.
    ///
    /// The default is to clear the menu.
    pub fn clear(mut self, val: bool) -> Self {
        self.clear = val;
        self
    }

    /// Sets initial selected element when select menu is rendered
    ///
    /// Element is indicated by the index at which it appears in [`item`](Self::item) method invocation or [`items`](Self::items) slice.
    pub fn default(mut self, val: usize) -> Self {
        self.default = val;
        self
    }

    /// Sets an optional max length for a page.
    ///
    /// Max length is disabled by None
    pub fn max_length(mut self, val: usize) -> Self {
        // Paging subtracts two from the capacity, paging does this to
        // make an offset for the page indicator. So to make sure that
        // we can show the intended amount of items we need to add two
        // to our value.
        self.max_length = Some(val + 2);
        self
    }

    /// Add a single item to the selector.
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// use dialoguer::Select;
    ///
    /// fn main() {
    ///     let selection = Select::new()
    ///         .item("Item 1")
    ///         .item("Item 2")
    ///         .interact()
    ///         .unwrap();
    /// }
    /// ```
    pub fn item<T: ToString>(mut self, item: T) -> Self {
        self.items.push(item.to_string());

        self
    }

    /// Adds multiple items to the selector.
    pub fn items<T, I>(mut self, items: I) -> Self
    where
        T: ToString,
        I: IntoIterator<Item = T>,
    {
        self.items
            .extend(items.into_iter().map(|item| item.to_string()));

        self
    }

    /// Sets the select prompt.
    ///
    /// By default, when a prompt is set the system also prints out a confirmation after
    /// the selection. You can opt-out of this with [`report`](Self::report).
    pub fn with_prompt<S: Into<String>>(mut self, prompt: S) -> Self {
        self.prompt = Some(prompt.into());
        self.report = true;
        self
    }

    /// Indicates whether to report the selected value after interaction.
    ///
    /// The default is to report the selection.
    pub fn report(mut self, val: bool) -> Self {
        self.report = val;
        self
    }

    /// Enables user interaction and returns the result.
    ///
    /// The user can select the items with the 'Space' bar or 'Enter' and the index of selected item will be returned.
    /// The dialog is rendered on stderr.
    /// Result contains `index` if user selected one of items using 'Enter'.
    /// This unlike [`interact_opt`](Self::interact_opt) does not allow to quit with 'Esc' or 'q'.
    #[inline]
    pub fn interact(self) -> Result<usize> {
        self.interact_on(&Term::stderr())
    }

    /// Enables user interaction and returns the result.
    ///
    /// The user can select the items with the 'Space' bar or 'Enter' and the index of selected item will be returned.
    /// The dialog is rendered on stderr.
    /// Result contains `Some(index)` if user selected one of items using 'Enter' or `None` if user cancelled with 'Esc' or 'q'.
    ///
    /// ## Example
    ///
    ///```rust,no_run
    /// use dialoguer::Select;
    ///
    /// fn main() {
    ///     let items = vec!["foo", "bar", "baz"];
    ///
    ///     let selection = Select::new()
    ///         .with_prompt("What do you choose?")
    ///         .items(&items)
    ///         .interact_opt()
    ///         .unwrap();
    ///
    ///     match selection {
    ///         Some(index) => println!("You chose: {}", items[index]),
    ///         None => println!("You did not choose anything.")
    ///     }
    /// }
    ///```
    #[inline]
    pub fn interact_opt(self) -> Result<Option<usize>> {
        self.interact_on_opt(&Term::stderr())
    }

    /// Like [`interact`](Self::interact) but allows a specific terminal to be set.
    #[inline]
    pub fn interact_on(self, mut term: &Term) -> Result<usize> {
        Ok(self
            ._interact_on(&mut term, false)?
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Quit not allowed in this case"))?)
    }

    /// Like [`interact_opt`](Self::interact_opt) but allows a specific terminal to be set.
    #[inline]
    pub fn interact_on_opt(self, mut term: &Term) -> Result<Option<usize>> {
        self._interact_on(&mut term, true)
    }

    #[inline]
    pub async fn interact_on_async(self, term: &mut impl TermAccess) -> Result<usize> {
        Ok(self
            ._interact_on_async(term, false).await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Quit not allowed in this case"))?)
    }

    /// Like [`interact_opt`](Self::interact_opt) but allows a specific terminal to be set.
    #[inline]
    pub async fn interact_on_opt_async(self, term: &mut impl TermAccess) -> Result<Option<usize>> {
        self._interact_on_async(term, true).await
    }

    /// Like `interact` but allows a specific terminal to be set.
    fn _interact_on(self, term_access: &mut impl TermAccess, allow_quit: bool) -> Result<Option<usize>> {
        let term = &term_access.term();
        let (mut paging, mut render, mut sel, size_vec) = self._interact_on_prepare(term)?;

        loop {
            self._interact_on_render(&mut paging, &mut render, sel)?;
            term.flush()?;

            let key = match term_access.read_key_blocking()  {
                AsRequested(key) => key,
                WriteLine(line) => {
                    render.clear()?;
                    term.write_line(&*line)?;
                    continue
                }
                mut err => {
                    err.try_into_error()?;
                    continue
                }
            };
            match self._interact_on_process_key(allow_quit, term, &mut paging, &mut render, &mut sel, &size_vec, key)? {
                Continue => continue,
                Break => break,
                Return(res) => return Ok(res)
            }
        }
        Err(Error::new(ErrorKind::Other, "Implementation mistake, should be unreachable").into())
    }

    async fn _interact_on_async(self, term_access: &mut impl TermAccess, allow_quit: bool) -> Result<Option<usize>> {
        let term = &term_access.term();
        let (mut paging, mut render, mut sel, size_vec) = self._interact_on_prepare(term)?;

        loop {
            self._interact_on_render(&mut paging, &mut render, sel)?;
            term.flush()?;

            let key = match term_access.read_key_async().await {
                AsRequested(key) => key,
                WriteLine(line) => {
                    render.clear()?;
                    term.write_line(&*line)?;
                    continue
                }
                mut err => {
                    err.try_into_error()?;
                    continue
                }
            };
            match self._interact_on_process_key(allow_quit, term, &mut paging, &mut render, &mut sel, &size_vec, key)? {
                Continue => continue,
                Break => break,
                Return(res) => return Ok(res)
            }
        }
        Err(Error::new(ErrorKind::Other, "Implementation mistake, should be unreachable").into())
    }

    #[inline]
    fn _interact_on_process_key(&self, allow_quit: bool, term: &Term, paging: &mut Paging, render: &mut TermThemeRenderer, sel: &mut usize, size_vec: &Vec<usize>, key: Key) -> Result<LoopStatement<Option<usize>>>  {
        match key {
            Key::ArrowDown | Key::Tab | Key::Char('j') => {
                if *sel == !0 {
                    *sel = 0;
                } else {
                    *sel = (*sel as u64 + 1).rem(self.items.len() as u64) as usize;
                }
            }
            Key::Escape | Key::Char('q') => {
                if allow_quit {
                    if self.clear {
                        render.clear()?;
                    } else {
                        term.clear_last_lines(paging.capacity)?;
                    }

                    term.show_cursor()?;
                    term.flush()?;

                    return Ok(Return(None));
                }
            }
            Key::ArrowUp | Key::BackTab | Key::Char('k') => {
                if *sel == !0 {
                    *sel = self.items.len() - 1;
                } else {
                    *sel = ((*sel as i64 - 1 + self.items.len() as i64)
                        % (self.items.len() as i64)) as usize;
                }
            }
            Key::ArrowLeft | Key::Char('h') => {
                if paging.active {
                    *sel = paging.previous_page();
                }
            }
            Key::ArrowRight | Key::Char('l') => {
                if paging.active {
                    *sel = paging.next_page();
                }
            }

            Key::Enter | Key::Char(' ') if *sel != !0 => {
                if self.clear {
                    render.clear()?;
                }

                if let Some(ref prompt) = self.prompt {
                    if self.report {
                        render.select_prompt_selection(prompt, &self.items[*sel])?;
                    }
                }

                term.show_cursor()?;
                term.flush()?;

                return Ok(Return(Some(*sel)));
            }
            _ => {}
        }

        paging.update(*sel)?;

        if paging.active {
            render.clear()?;
        } else {
            render.clear_preserve_prompt(&size_vec)?;
        }
        Ok(Continue)
    }

    #[inline]
    fn _interact_on_render(&self, paging: &mut Paging, render: &mut TermThemeRenderer, sel: usize) -> Result<()> {
        if let Some(ref prompt) = self.prompt {
            paging.render_prompt(|paging_info| render.select_prompt(prompt, paging_info))?;
        }

        for (idx, item) in self
            .items
            .iter()
            .enumerate()
            .skip(paging.current_page * paging.capacity)
            .take(paging.capacity)
        {
            render.select_prompt_item(item, sel == idx)?;
        }
        Ok(())
    }

    #[inline]
    fn _interact_on_prepare<'a>(&'a self, term: &'a Term) -> io::Result<(Paging<'a>, TermThemeRenderer<'a>, usize, Vec<usize>)> {
        if !term.is_term() {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "not a terminal").into());
        }

        if self.items.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Empty list of items given to `Select`",
            ))?;
        }

        let mut paging = Paging::new(term, self.items.len(), self.max_length);
        let mut render = TermThemeRenderer::new(term, self.theme);
        let mut sel = self.default;

        let mut size_vec = Vec::new();

        for items in self
            .items
            .iter()
            .flat_map(|i| i.split('\n'))
            .collect::<Vec<_>>()
        {
            let size = &items.len();
            size_vec.push(*size);
        }

        term.hide_cursor()?;
        paging.update_page(sel);
        Ok((paging, render, sel, size_vec))
    }
}

impl<'a> Select<'a> {
    /// Creates a select prompt with a specific theme.
    ///
    /// ## Example
    ///
    /// ```rust,no_run
    /// use dialoguer::{theme::ColorfulTheme, Select};
    ///
    /// fn main() {
    ///     let selection = Select::with_theme(&ColorfulTheme::default())
    ///         .items(&["foo", "bar", "baz"])
    ///         .interact()
    ///         .unwrap();
    /// }
    /// ```
    pub fn with_theme(theme: &'a dyn Theme) -> Self {
        Self {
            default: !0,
            items: vec![],
            prompt: None,
            report: false,
            clear: true,
            max_length: None,
            theme,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone() {
        let select = Select::new().with_prompt("Do you want to continue?");

        let _ = select.clone();
    }

    #[test]
    fn test_str() {
        let selections = &[
            "Ice Cream",
            "Vanilla Cupcake",
            "Chocolate Muffin",
            "A Pile of sweet, sweet mustard",
        ];

        assert_eq!(
            Select::new().default(0).items(&selections[..]).items,
            selections
        );
    }

    #[test]
    fn test_string() {
        let selections = vec!["a".to_string(), "b".to_string()];

        assert_eq!(
            Select::new().default(0).items(&selections).items,
            selections
        );
    }

    #[test]
    fn test_ref_str() {
        let a = "a";
        let b = "b";

        let selections = &[a, b];

        assert_eq!(Select::new().default(0).items(selections).items, selections);
    }

    #[test]
    fn test_iterator() {
        let items = ["First", "Second", "Third"];
        let iterator = items.iter().skip(1);

        assert_eq!(Select::new().default(0).items(iterator).items, &items[1..]);
    }
}
