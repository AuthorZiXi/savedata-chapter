export interface MenuItem {
    label?: string;
    danger?: boolean;
    separator?: boolean;
    action?: () => void;
}

export interface Dialog {
    title: string;
    body: string;
    confirmText?: string;
    danger?: boolean;
    onConfirm: () => void | Promise<void>;
}