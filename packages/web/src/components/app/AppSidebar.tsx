import { Link } from '@tanstack/solid-router';
import packageJson from '../../../package.json';
import {
	Sidebar,
	SidebarContent,
	SidebarFooter,
	SidebarGroup,
	SidebarHeader,
	SidebarMenu,
	SidebarMenuButton,
	SidebarMenuItem,
    SidebarRail,
} from '@repo/ui-solid/base/sidebar';
import { openSettings, SettingsModal } from './settings/settings';

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger
} from "@repo/ui-solid/base/dialog"
import { Button } from '@repo/ui-solid/base/button';
import { TextField, TextFieldInput, TextFieldLabel } from '@repo/ui-solid/base/text-field';
import { Settings } from 'lucide-solid';
import { TooltipX } from '@repo/ui-solid/custom/tooltip';
 
export function DialogDemo() {
  return (
    <Dialog>
      <DialogTrigger as={Button<"button">}>Edit Profile</DialogTrigger>
      <DialogContent class="sm:max-w-[425px]">
        <DialogHeader>
          <DialogTitle>Edit profile</DialogTitle>
          <DialogDescription>
            Make changes to your profile here. Click save when you're done.
          </DialogDescription>
        </DialogHeader>
        <div class="grid gap-4 py-4">
          <TextField class="grid grid-cols-4 items-center gap-4">
            <TextFieldLabel class="text-right">Name</TextFieldLabel>
            <TextFieldInput value="Pedro Duarte" class="col-span-3" type="text" />
          </TextField>
          <TextField class="grid grid-cols-4 items-center gap-4">
            <TextFieldLabel class="text-right">Username</TextFieldLabel>
            <TextFieldInput value="@peduarte" class="col-span-3" type="text" />
          </TextField>
        </div>
        <DialogFooter>
          <Button type="submit">Save changes</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

export function AppSidebar() {
	return (
		<Sidebar>
			<SidebarHeader class="flex-row">
				<TooltipX content={`Version ${packageJson.version}`}>
					<Link to="/">
						<h1 class="flex gap-1">
							<span>aa</span>
							<span class="text-muted-foreground">bot</span>
						</h1>
					</Link>
				</TooltipX>
			</SidebarHeader>
			<SidebarContent>
				<SidebarGroup >
				</SidebarGroup>
				<SidebarGroup />
			</SidebarContent>
			<SidebarFooter>
				<SidebarMenuButton onClick={()=> openSettings()}><Settings /></SidebarMenuButton>
				{/* <DialogDemo /> */}
			</SidebarFooter>
			<SidebarRail />
		</Sidebar>
	);
}
