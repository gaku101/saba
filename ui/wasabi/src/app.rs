use crate::alloc::string::ToString;
use crate::cursor::Cursor;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::cell::RefCell;
use noli::error::Result as OsResult;
use noli::prelude::SystemApi;
use noli::println;
use noli::rect::Rect;
use noli::sys::api::MouseEvent;
use noli::sys::wasabi::Api;
use noli::window::StringSize;
use noli::window::Window;
use saba_core::browser::Browser;
use saba_core::constants::WHITE;
use saba_core::constants::WINDOW_HEIGHT;
use saba_core::constants::WINDOW_INIT_X_POS;
use saba_core::constants::WINDOW_INIT_Y_POS;
use saba_core::constants::WINDOW_WIDTH;
use saba_core::constants::*;
use saba_core::display_item::DisplayItem;
use saba_core::error::Error;
use saba_core::http::HttpResponse;
use saba_core::renderer::layout::computed_style::FontSize;
use saba_core::renderer::layout::computed_style::TextDecoration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputMode {
    Normal,
    Editing,
}
#[derive(Debug)]
pub struct WasabiUI {
    browser: Rc<RefCell<Browser>>,
    input_url: String,
    input_mode: InputMode,
    window: Window,
    cursor: Cursor,
}

impl WasabiUI {
    pub fn new(browser: Rc<RefCell<Browser>>) -> Self {
        Self {
            browser,
            input_url: String::new(),
            input_mode: InputMode::Normal,
            window: Window::new(
                "saba".to_string(),
                WHITE,
                WINDOW_INIT_X_POS,
                WINDOW_INIT_Y_POS,
                WINDOW_WIDTH,
                WINDOW_HEIGHT,
            )
            .unwrap(),
            cursor: Cursor::new(),
        }
    }

    pub fn start(
        &mut self,
        handle_url: fn(String) -> Result<HttpResponse, Error>,
    ) -> Result<(), Error> {
        self.setup()?;

        self.run_app(handle_url)?;

        Ok(())
    }

    fn run_app(
        &mut self,
        handle_url: fn(String) -> Result<HttpResponse, Error>,
    ) -> Result<(), Error> {
        loop {
            self.handle_mouse_input(handle_url)?;
            self.handle_key_input(handle_url)?;
        }
    }

    fn handle_mouse_input(
        &mut self,
        handle_url: fn(String) -> Result<HttpResponse, Error>,
    ) -> Result<(), Error> {
        if let Some(MouseEvent { button, position }) = Api::get_mouse_cursor_info() {
            self.window.flush_area(self.cursor.rect());
            self.cursor.set_position(position.x, position.y);
            self.window.flush_area(self.cursor.rect());
            self.cursor.flush();

            if button.l() || button.c() || button.r() {
                // 相対位置を計算する
                let relative_pos = (
                    position.x - WINDOW_INIT_X_POS,
                    position.y - WINDOW_INIT_Y_POS,
                );

                // ウィンドウの外をクリックされたときは何もしない
                if relative_pos.0 < 0
                    || relative_pos.0 > WINDOW_WIDTH
                    || relative_pos.1 < 0
                    || relative_pos.1 > WINDOW_HEIGHT
                {
                    println!("button clicked OUTSIDE window: {button:?} {position:?}");

                    return Ok(());
                }

                // ツールバーの範囲をクリックされたとき、InputModeをEditingに変更する
                if relative_pos.1 < TOOLBAR_HEIGHT + TITLE_BAR_HEIGHT
                    && relative_pos.1 >= TITLE_BAR_HEIGHT
                {
                    self.clear_address_bar()?;
                    self.input_url = String::new();
                    self.input_mode = InputMode::Editing;
                    println!("button clicked in toolbar: {button:?} {position:?}");
                    return Ok(());
                }

                self.input_mode = InputMode::Normal;

                let position_in_content_area = (
                    relative_pos.0,
                    relative_pos.1 - TITLE_BAR_HEIGHT - TOOLBAR_HEIGHT,
                );
                let page = self.browser.borrow().current_page();
                let next_destination = page.borrow_mut().clicked(position_in_content_area);

                if let Some(url) = next_destination {
                    self.input_url = url.clone();
                    self.update_address_bar()?;
                    self.start_navigation(handle_url, url)?;
                }
            }
        }

        Ok(())
    }

    fn handle_key_input(
        &mut self,
        handle_url: fn(String) -> Result<HttpResponse, Error>,
    ) -> Result<(), Error> {
        match self.input_mode {
            InputMode::Normal => {
                // InputModeがNormalのとき、キー入力を無視する
                let _ = Api::read_key();
            }
            InputMode::Editing => {
                if let Some(c) = Api::read_key() {
                    if c == 0x0A as char {
                        // エンターキーが押されたので、ナビゲーションを開始する
                        self.start_navigation(handle_url, self.input_url.clone())?;

                        self.input_url = String::new();
                        self.input_mode = InputMode::Normal;
                    } else if c == 0x7F as char || c == 0x08 as char {
                        // デリートキーまたはバックスペースキーが押されたので、最後の文字を削除する
                        self.input_url.pop();
                        self.update_address_bar()?;
                    } else {
                        self.input_url.push(c);
                        self.update_address_bar()?;
                    }
                }
            }
        }

        Ok(())
    }

    fn setup(&mut self) -> Result<(), Error> {
        if let Err(error) = self.setup_toolbar() {
            // OsResultとResultが持つError型は異なるので、変換する
            return Err(Error::InvalidUI(format!(
                "failed to initialize a toolbar with error: {:#?}",
                error
            )));
        }
        // 画面を更新する
        self.window.flush();
        Ok(())
    }

    fn setup_toolbar(&mut self) -> OsResult<()> {
        // ツールバーの背景の四角を描画
        self.window
            .fill_rect(LIGHTGREY, 0, 0, WINDOW_WIDTH, TOOLBAR_HEIGHT)?;

        // ツールバーとコンテンツエリアの境目の線を描画
        self.window
            .draw_line(GREY, 0, TOOLBAR_HEIGHT, WINDOW_WIDTH - 1, TOOLBAR_HEIGHT)?;
        self.window.draw_line(
            DARKGREY,
            0,
            TOOLBAR_HEIGHT + 1,
            WINDOW_WIDTH - 1,
            TOOLBAR_HEIGHT + 1,
        )?;

        // アドレスバーの横に"Address:"という文字列を描画
        self.window.draw_string(
            BLACK,
            5,
            5,
            "Address:",
            StringSize::Medium,
            /*underline=*/ false,
        )?;

        // アドレスバーの四角を描画
        self.window
            .fill_rect(WHITE, 70, 2, WINDOW_WIDTH - 74, 2 + ADDRESSBAR_HEIGHT)?;

        // アドレスバーの影の線を描画
        self.window.draw_line(GREY, 70, 2, WINDOW_WIDTH - 4, 2)?;
        self.window
            .draw_line(GREY, 70, 2, 70, 2 + ADDRESSBAR_HEIGHT)?;
        self.window.draw_line(BLACK, 71, 3, WINDOW_WIDTH - 5, 3)?;

        self.window
            .draw_line(GREY, 71, 3, 71, 1 + ADDRESSBAR_HEIGHT)?;

        Ok(())
    }
}
