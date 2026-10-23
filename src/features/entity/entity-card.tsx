import { Handle, Position } from "@xyflow/react";
import { CalendarDays } from "lucide-react";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  HoverCard,
  HoverCardContent,
  HoverCardTrigger,
} from "@/components/ui/hover-card";
import type { EntityData } from "./model";
// 入参只要 data：既当 React Flow 节点，也能当普通卡片用
export function EntityNode({ data }: { data: EntityData }) {
  return (
    <>
      {/* 连线锚点：父级从底部出、子级从顶部入；藏掉但保留挂点，且不许用户拖出新连线 */}
      <Handle
        type="target"
        position={Position.Top}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <Handle
        type="source"
        position={Position.Bottom}
        isConnectable={false}
        className="!border-0 !bg-transparent"
      />
      <EntityCard data={data} />
    </>
  );
}

function entityInitials(id: string): string {
  const parts = id
    .trim()
    .split(/[\s·._-]+/u)
    .filter(Boolean);
  if (parts.length > 1) {
    return parts
      .slice(0, 2)
      .map((part) => Array.from(part)[0])
      .join("")
      .toLocaleUpperCase();
  }
  return Array.from(parts[0] ?? "?")
    .slice(0, 2)
    .join("")
    .toLocaleUpperCase();
}

function EntityCard({ data }: { data: EntityData }) {
  return (
    <Card className="entity-card w-[280px] gap-0 overflow-hidden py-0">
      <CardHeader className="gap-3 p-4">
        <div className="flex items-start gap-3">
          <Avatar className="size-10 shrink-0 overflow-hidden">
            {data.avatar && (
              <AvatarImage
                src={data.avatar}
                alt={data.name}
                className="object-cover"
              />
            )}
            <AvatarFallback className="text-xs font-medium">
              {entityInitials(data.name)}
            </AvatarFallback>
          </Avatar>
          <CardTitle className="min-w-0 self-center break-words text-base leading-6">
            {data.name}
          </CardTitle>
        </div>
        {data.description && (
          <CardDescription className="whitespace-pre-wrap break-words leading-6">
            {data.description}
          </CardDescription>
        )}
      </CardHeader>
      {(data.baseTime ||
        data.extraProps.length > 0 ||
        data.parents.length > 0) && (
        <CardContent className="space-y-3 border-t bg-muted/30 p-4">
          {data.baseTime && (
            <div className="flex items-start gap-2 text-xs text-muted-foreground">
              <CalendarDays
                className="mt-0.5 size-3.5 shrink-0"
                aria-hidden="true"
              />
              <span className="break-words leading-5">{data.baseTime}</span>
            </div>
          )}
          {data.extraProps.map(([key, value]) => (
            <div key={key} className="flex items-baseline gap-2 text-xs">
              <span className="shrink-0 text-muted-foreground">{key}</span>
              <span className="min-w-0 break-words leading-5">{value}</span>
            </div>
          ))}
          {data.parents.length > 0 && (
            <div className="space-y-2">
              <p className="text-xs text-muted-foreground">
                {data.belongLabel}
              </p>
              <div className="flex flex-wrap gap-1.5">
                {data.parents.map((p) =>
                  p.preview ? (
                    <HoverCard key={p.id} openDelay={250}>
                      <HoverCardTrigger asChild>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="nodrag max-w-full rounded-full text-left outline-none focus-visible:ring-2 focus-visible:ring-ring"
                        >
                          <Badge
                            variant="outline"
                            className="max-w-full cursor-pointer whitespace-normal break-words bg-background px-2 py-0.5 font-normal hover:bg-accent"
                          >
                            {p.name}
                          </Badge>
                        </Button>
                      </HoverCardTrigger>
                      <HoverCardContent
                        side="right"
                        align="start"
                        sideOffset={12}
                        className="nodrag nopan w-auto border-0 bg-transparent p-0 shadow-none"
                      >
                        <EntityCard data={p.preview} />
                      </HoverCardContent>
                    </HoverCard>
                  ) : (
                    <Badge
                      key={p.id}
                      variant="outline"
                      className="max-w-full whitespace-normal break-words bg-background px-2 py-0.5 font-normal"
                    >
                      {p.name}
                    </Badge>
                  ),
                )}
              </div>
            </div>
          )}
        </CardContent>
      )}
    </Card>
  );
}
