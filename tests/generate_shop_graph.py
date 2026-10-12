"""Rebuild the deterministic 100-entity, 250-relation shop demonstration fixture."""

import json
from pathlib import Path


# Each entity has two domain relationships; additional cross-domain links follow.
ENTITIES = [
    ("customer", "Покупатель", "name email", "order cart"),
    ("order", "Заказ", "created_at status total", "order_item payment"),
    ("order_item", "Позиция заказа", "quantity unit_price", "product tax_rate"),
    ("product", "Товар", "name price stock", "category brand"),
    ("category", "Категория", "name", "catalog seo_page"),
    ("payment", "Оплата", "amount method status", "payment_method transaction"),
    ("delivery", "Доставка", "address tracking_number status", "shipment carrier"),
    ("cart", "Корзина", "created_at total", "order_item customer"),
    ("wishlist", "Список желаний", "name visibility", "customer product"),
    ("review", "Отзыв", "rating text", "customer product"),
    ("catalog", "Каталог", "name version", "product price_list"),
    ("brand", "Бренд", "name country", "supplier media_asset"),
    ("product_variant", "Вариант товара", "sku size color", "product inventory_item"),
    ("product_attribute", "Характеристика товара", "name value", "product attribute_definition"),
    ("attribute_definition", "Тип характеристики", "name data_type", "category localization"),
    ("media_asset", "Медиафайл", "url mime_type", "document product"),
    ("price_list", "Прайс-лист", "name valid_from", "price_entry currency"),
    ("price_entry", "Цена товара", "amount valid_until", "product_variant tax_rate"),
    ("bundle", "Комплект", "name discount", "product product_variant"),
    ("seo_page", "Страница SEO", "slug title", "category localization"),
    ("warehouse", "Склад", "name address", "warehouse_zone employee"),
    ("warehouse_zone", "Зона склада", "code temperature", "bin_location warehouse"),
    ("bin_location", "Ячейка хранения", "code capacity", "inventory_item warehouse_zone"),
    ("inventory_item", "Остаток товара", "quantity reserved", "product_variant warehouse"),
    ("stock_movement", "Движение запасов", "quantity occurred_at", "inventory_item employee"),
    ("stock_reservation", "Резерв товара", "quantity expires_at", "inventory_item order_item"),
    ("supplier", "Поставщик", "name email", "purchase_order supplier_contract"),
    ("purchase_order", "Закупка", "number status", "purchase_item warehouse"),
    ("purchase_item", "Позиция закупки", "quantity cost", "product_variant supplier"),
    ("supplier_contract", "Договор поставки", "number valid_until", "supplier document"),
    ("shipment", "Отправление", "tracking_number status", "shipment_item delivery_route"),
    ("shipment_item", "Позиция отправления", "quantity weight", "order_item package"),
    ("carrier", "Перевозчик", "name tracking_url", "shipping_rate supplier_contract"),
    ("delivery_route", "Маршрут доставки", "distance duration", "route_stop vehicle"),
    ("route_stop", "Остановка маршрута", "sequence eta", "address pickup_point"),
    ("vehicle", "Транспорт", "plate capacity", "driver carrier"),
    ("driver", "Водитель", "name license", "employee vehicle"),
    ("pickup_point", "Пункт выдачи", "name opening_hours", "address employee"),
    ("package", "Упаковка", "weight dimensions", "shipment product_attribute"),
    ("shipping_rate", "Тариф доставки", "amount zone", "carrier currency"),
    ("payment_method", "Способ оплаты", "name enabled", "payment_gateway currency"),
    ("payment_gateway", "Платёжный шлюз", "name endpoint", "gateway_account webhook"),
    ("gateway_account", "Аккаунт шлюза", "merchant_id status", "currency organization"),
    ("transaction", "Транзакция", "reference status amount", "payment_gateway ledger_entry"),
    ("refund", "Возврат платежа", "amount reason", "payment return_request"),
    ("invoice", "Счёт", "number amount due_at", "order organization"),
    ("tax_rate", "Налоговая ставка", "name percent", "tax_region currency"),
    ("tax_region", "Налоговый регион", "code name", "country organization"),
    ("currency", "Валюта", "code symbol", "price_entry country"),
    ("ledger_entry", "Проводка", "debit credit", "gateway_account invoice"),
    ("promotion", "Акция", "name start_at end_at", "promotion_rule campaign"),
    ("promotion_rule", "Правило акции", "condition discount", "category customer_segment"),
    ("coupon", "Купон", "code discount expires_at", "promotion customer"),
    ("campaign", "Кампания", "name budget", "channel audience"),
    ("channel", "Канал продаж", "name kind", "organization analytics_event"),
    ("audience", "Аудитория", "name size", "customer_segment campaign"),
    ("customer_segment", "Сегмент клиентов", "name criteria", "customer loyalty_program"),
    ("loyalty_program", "Программа лояльности", "name point_rate", "loyalty_account reward"),
    ("loyalty_account", "Счёт лояльности", "points tier", "customer transaction"),
    ("reward", "Награда", "name cost_points", "product coupon"),
    ("support_ticket", "Обращение поддержки", "subject priority status", "customer ticket_message"),
    ("ticket_message", "Сообщение поддержки", "text created_at", "employee document"),
    ("return_request", "Заявка на возврат", "reason status", "order return_item"),
    ("return_item", "Возвращаемая позиция", "quantity condition", "order_item product_variant"),
    ("warranty", "Гарантия", "expires_at terms", "product service_case"),
    ("service_case", "Сервисный случай", "description status", "customer repair_order"),
    ("repair_order", "Ремонт", "status cost", "employee spare_part"),
    ("spare_part", "Запчасть", "sku quantity", "product_variant supplier"),
    ("knowledge_article", "Статья базы знаний", "title body", "category localization"),
    ("feedback", "Обратная связь", "score text", "customer support_ticket"),
    ("organization", "Организация", "name registration_number", "department address"),
    ("department", "Отдел", "name cost_center", "employee ledger_entry"),
    ("employee", "Сотрудник", "name email", "user_account role"),
    ("user_account", "Учётная запись", "login enabled", "session authentication_policy"),
    ("role", "Роль доступа", "name description", "permission user_account"),
    ("permission", "Разрешение", "resource action", "authentication_policy audit_event"),
    ("session", "Сессия", "created_at expires_at", "device audit_event"),
    ("device", "Устройство", "type fingerprint", "customer user_account"),
    ("authentication_policy", "Политика доступа", "mfa timeout", "organization customer_segment"),
    ("audit_event", "Событие аудита", "action occurred_at", "employee analytics_event"),
    ("notification", "Уведомление", "subject status", "customer notification_template"),
    ("notification_template", "Шаблон уведомления", "name body", "localization channel"),
    ("email_message", "Письмо", "recipient status", "notification document"),
    ("sms_message", "SMS", "phone status", "notification integration"),
    ("webhook", "Вебхук", "url enabled", "integration integration_event"),
    ("integration", "Интеграция", "name endpoint", "organization api_credential"),
    ("integration_event", "Событие интеграции", "type status", "order message_queue"),
    ("message_queue", "Очередь сообщений", "name backlog", "worker_job monitoring_metric"),
    ("worker_job", "Фоновая задача", "type progress", "integration_event authentication_policy"),
    ("api_credential", "Ключ API", "key_id expires_at", "integration role"),
    ("address", "Адрес", "street postal_code", "city customer"),
    ("city", "Город", "name region", "country tax_region"),
    ("country", "Страна", "code name", "localization currency"),
    ("localization", "Локализация", "locale timezone", "translation organization"),
    ("translation", "Перевод", "key text", "notification_template knowledge_article"),
    ("document", "Документ", "number file_url", "media_asset organization"),
    ("analytics_event", "Событие аналитики", "type timestamp", "customer analytics_report"),
    ("analytics_report", "Отчёт аналитики", "name period", "dashboard monitoring_metric"),
    ("dashboard", "Панель мониторинга", "name refresh_interval", "employee analytics_report"),
    ("monitoring_metric", "Метрика мониторинга", "name value", "notification worker_job"),
]

EXTRA_LINKS = [
    ("order", "delivery"), ("customer", "wishlist"), ("customer", "customer_segment"),
    ("customer", "address"), ("customer", "loyalty_account"),
    ("order", "invoice"), ("order", "coupon"), ("order", "stock_reservation"),
    ("order", "channel"), ("order", "support_ticket"),
    ("product", "product_variant"), ("product", "review"), ("product", "warranty"),
    ("product", "media_asset"), ("product", "bundle"),
    ("category", "promotion_rule"), ("payment", "refund"), ("payment", "invoice"),
    ("delivery", "address"), ("delivery", "package"), ("delivery", "pickup_point"),
    ("cart", "coupon"), ("review", "media_asset"), ("catalog", "channel"),
    ("supplier", "address"), ("purchase_order", "invoice"), ("warehouse", "address"),
    ("stock_movement", "purchase_order"), ("shipment", "carrier"),
    ("carrier", "integration"), ("vehicle", "monitoring_metric"),
    ("transaction", "refund"), ("invoice", "tax_rate"), ("promotion", "product"),
    ("campaign", "notification"), ("coupon", "order"), ("loyalty_account", "reward"),
    ("support_ticket", "order"), ("return_request", "refund"),
    ("return_item", "warehouse"), ("repair_order", "invoice"),
    ("employee", "department"), ("user_account", "customer"), ("role", "organization"),
    ("notification", "email_message"), ("notification", "sms_message"),
    ("integration_event", "payment"), ("worker_job", "audit_event"),
    ("analytics_report", "order"), ("dashboard", "monitoring_metric"),
]

ORIGINAL_IDS = {
    ("customer", "order"): "customer_orders", ("order", "order_item"): "order_items",
    ("order_item", "product"): "item_product", ("product", "category"): "product_category",
    ("order", "payment"): "order_payments", ("order", "delivery"): "order_deliveries",
}


def build_graph():
    entities = [{"id": id_, "name": name, "attributes": ["id", *attributes.split()]}
                for id_, name, attributes, _ in ENTITIES]
    pairs = [(id_, target)
             for id_, _, _, targets in ENTITIES for target in targets.split()]
    pairs.extend(EXTRA_LINKS)
    ids = {node["id"] for node in entities}
    assert len(entities) == len(ids) == 100
    assert len(pairs) == len(set(pairs)) == 250
    assert all(source in ids and target in ids for source, target in pairs)
    pairs.sort(key=lambda pair: (
        list(ORIGINAL_IDS).index(pair) if pair in ORIGINAL_IDS else len(ORIGINAL_IDS),
        pair if pair not in ORIGINAL_IDS else ("", ""),
    ))
    relations = [{"id": ORIGINAL_IDS.get((source, target), f"{source}_to_{target}"),
                  "source": source, "target": target, "name": "связан с", "cardinality": "N:1"}
                 for source, target in pairs]
    for relation, name in zip(relations, (
            "оформляет", "содержит", "ссылается на", "относится к",
            "имеет попытки оплаты", "отправляется доставками")):
        relation.update(name=name, cardinality="1:N" if relation["id"] in (
            "customer_orders", "order_items", "order_payments", "order_deliveries") else "N:1")
    return {"graph": {"name": "Интернет-магазин: полная экосистема", "directed": True,
                      "description": "Каталог, заказы, склад, логистика, финансы, маркетинг и поддержка",
                      "entity_count": 100, "relation_count": 250},
            "entities": entities, "relations": relations}


def main():
    graph = build_graph()
    lines = ["[graph]"]
    for key, value in graph["graph"].items():
        lines.append(f"{key} = {json.dumps(value, ensure_ascii=False)}")
    for section in ("entities", "relations"):
        for record in graph[section]:
            lines.extend(["", f"[[{section}]]"])
            lines.extend(f"{key} = {json.dumps(value, ensure_ascii=False)}" for key, value in record.items())
    path = Path(__file__).parent / "fixtures" / "Graph.toml"
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote 100 entities and 250 relations to {path}")


if __name__ == "__main__":
    main()
