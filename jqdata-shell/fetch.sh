#!/bin/bash

if [ -z "$JQDATA_MOB" ]; then
    echo JQDATA_MOB must be set
    exit 4
fi

if [ -z "$JQDATA_PWD" ]; then
    echo JQDATA_PWD must be set
    exit 4
fi

JQDATA_TOKEN=$(curl -s https://dataapi.joinquant.com/apis -H 'Content-Type: application/json' -d "{\"method\":\"get_current_token\",\"mob\":\"$JQDATA_MOB\",\"pwd\":\"$JQDATA_PWD\"}")
#echo token acquired: $JQDATA_TOKEN

curl -kv -s https://dataapi.joinquant.com/apis -H 'Content-Type: application/json' \
-d "{\"method\":\"get_price_period\",\"token\":\"$JQDATA_TOKEN\",\"unit\":\"1d\",\"code\":\"002415.XSHE\",\"date\":\"2019-12-02 00:00:00\",\"end_date\":\"2020-01-02 23:59:59\"}"

# 查询所有股票
#curl -s https://dataapi.joinquant.com/apis -H 'Content-Type: application/json' \
#-d "{\"method\":\"get_all_securities\",\"token\":\"$JQDATA_TOKEN\",\"code\":\"stock\"}"


