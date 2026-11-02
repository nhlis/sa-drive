import type { IconName } from "./icons";

export interface MenuItem {
  label: string;
  icon?: IconName;
  run: () => void;
  danger?: boolean;
  disabled?: boolean;
  separatorBefore?: boolean;
}
