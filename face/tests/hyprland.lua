-- The nested Hyprland of the face's checks: production-like, no animations,
-- the mock's desk colour behind the surface.
hl.config({
  general = { gaps_in = 0, gaps_out = 0, border_size = 0 },
  decoration = { rounding = 0, blur = { enabled = false }, shadow = { enabled = false } },
  animations = { enabled = false },
  input = { kb_layout = "us", follow_mouse = 1 },
  cursor = { no_warps = true, invisible = true },
  misc = { disable_hyprland_logo = true, disable_splash_rendering = true, force_default_wallpaper = 0, background_color = 0xff3b6896 },
})
